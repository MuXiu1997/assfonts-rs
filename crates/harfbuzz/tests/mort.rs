use assfonts_core::{FontFace, Subsetter};
use assfonts_harfbuzz::HarfBuzz;
use std::{collections::BTreeMap, sync::Arc};
use ttf_parser::{Face, Tag};

const BASE: &[u8] = include_bytes!("../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf");
fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(b[at..at + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
fn put16(b: &mut Vec<u8>, n: u16) {
    b.extend(n.to_be_bytes());
}
fn put32(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_be_bytes());
}
fn checksum(b: &[u8]) -> u32 {
    b.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

// Construct a synthetic SFNT around the vendored OFL fixture, without a new
// font binary. This also exercises normal table checksums and head adjustment.
fn with_mort(mort: &[u8]) -> Vec<u8> {
    let mut tables = BTreeMap::new();
    for i in 0..usize::from(u16_at(BASE, 4)) {
        let at = 12 + i * 16;
        let tag: [u8; 4] = BASE[at..at + 4].try_into().unwrap();
        let off = u32_at(BASE, at + 8) as usize;
        let len = u32_at(BASE, at + 12) as usize;
        let mut data = BASE[off..off + len].to_vec();
        if tag == *b"head" {
            data[8..12].fill(0);
        }
        tables.insert(tag, data);
    }
    tables.insert(*b"mort", mort.to_vec());
    let n = tables.len() as u16;
    let selector = n.ilog2() as u16;
    let search = (1 << selector) * 16;
    let mut output = vec![];
    put32(&mut output, 0x10000);
    for value in [n, search, selector, n * 16 - search] {
        put16(&mut output, value);
    }
    output.resize(12 + tables.len() * 16, 0);
    let mut head = 0;
    for (i, (tag, data)) in tables.iter().enumerate() {
        let off = output.len();
        if tag == b"head" {
            head = off;
        }
        let at = 12 + i * 16;
        output[at..at + 4].copy_from_slice(tag);
        output[at + 4..at + 8].copy_from_slice(&checksum(data).to_be_bytes());
        output[at + 8..at + 12].copy_from_slice(&(off as u32).to_be_bytes());
        output[at + 12..at + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend(data);
        output.resize(output.len().next_multiple_of(4), 0);
    }
    let adjust = 0xb1b0afbau32.wrapping_sub(checksum(&output));
    output[head + 8..head + 12].copy_from_slice(&adjust.to_be_bytes());
    output
}

fn mort(lookup: &[u8], coverage: u16) -> Vec<u8> {
    let mut b = vec![];
    put32(&mut b, 0x10000);
    put32(&mut b, 1);
    put32(&mut b, 1); // chain default flags
    put32(&mut b, 20 + lookup.len() as u32);
    put16(&mut b, 0); // feature count
    put16(&mut b, 1); // subtable count
    put16(&mut b, 8 + lookup.len() as u16);
    put16(&mut b, coverage);
    put32(&mut b, 1);
    b.extend(lookup);
    b
}
fn lookup6(pairs: &[(u16, u16)]) -> Vec<u8> {
    let mut pairs = pairs.to_vec();
    pairs.sort();
    let n = pairs.len() as u16;
    let selector = n.ilog2() as u16;
    let search = (1 << selector) * 4;
    let mut b = vec![];
    for value in [6, 4, n, search, selector, n * 4 - search] {
        put16(&mut b, value);
    }
    for (from, to) in pairs {
        put16(&mut b, from);
        put16(&mut b, to);
    }
    b
}
fn subset(mort: &[u8]) -> assfonts_core::Result<Vec<u8>> {
    HarfBuzz::default().subset(
        &FontFace {
            source: "synthetic mort fixture".into(),
            data: Arc::from(with_mort(mort)),
            index: 0,
        },
        &['a'].into(),
    )
}

#[test]
fn preserves_aat_path_closes_output_glyphs_and_remaps_compact_ids() {
    let source = Face::parse(BASE, 0).unwrap();
    let a = source.glyph_index('a').unwrap().0;
    let zhe = source.glyph_index('Ж').unwrap().0;
    let ya = source.glyph_index('Я').unwrap().0;
    let original = mort(&lookup6(&[(a, zhe), (zhe, ya)]), 0x8004);
    let output = subset(&original).unwrap();
    let face = Face::parse(&output, 0).unwrap();
    assert!(face.number_of_glyphs() < source.number_of_glyphs());
    let data = face.raw_face().table(Tag::from_bytes(b"mort")).unwrap();
    assert_eq!(u16_at(data, 22), 0x8004); // vertical coverage retained
    assert_eq!(u16_at(data, 28), 6); // compact lookup format
    let mut map = BTreeMap::new();
    for i in 0..usize::from(u16_at(data, 32)) {
        map.insert(u16_at(data, 40 + i * 4), u16_at(data, 42 + i * 4));
    }
    assert_eq!(map.len(), 2);
    let new_a = face.glyph_index('a').unwrap().0;
    let new_zhe = map[&new_a];
    let new_ya = map[&new_zhe];
    assert_ne!(new_zhe, zhe, "glyph IDs should actually be compacted");
    for (old, new) in [(zhe, new_zhe), (ya, new_ya)] {
        assert!(new < face.number_of_glyphs());
        assert_eq!(
            face.glyph_bounding_box(ttf_parser::GlyphId(new)),
            source.glyph_bounding_box(ttf_parser::GlyphId(old))
        );
    }
    assert_eq!(subset(&original).unwrap(), output);
}

#[test]
fn handles_segment_and_trimmed_lookups_and_deleted_glyphs() {
    let source = Face::parse(BASE, 0).unwrap();
    let a = source.glyph_index('a').unwrap().0;
    let target = source.glyph_index('Ж').unwrap().0;
    for format in [0, 2, 4, 6, 8, 10] {
        let mut lookup = vec![];
        put16(&mut lookup, format);
        match format {
            0 => {
                for g in 0..source.number_of_glyphs() {
                    put16(&mut lookup, if g == a { target } else { g });
                }
            }
            2 | 4 => {
                for value in [6, 1, 6, 0, 0, a, a, if format == 4 { 18 } else { target }] {
                    put16(&mut lookup, value);
                }
                if format == 4 {
                    put16(&mut lookup, target);
                }
            }
            6 => lookup = lookup6(&[(a, target)]),
            8 => {
                for value in [a, 1, target] {
                    put16(&mut lookup, value);
                }
            }
            10 => {
                for value in [2, a, 1, target] {
                    put16(&mut lookup, value);
                }
            }
            _ => unreachable!(),
        }
        let output = subset(&mort(&lookup, 4)).unwrap();
        let face = Face::parse(&output, 0).unwrap();
        let data = face.raw_face().table(Tag::from_bytes(b"mort")).unwrap();
        assert_eq!(u16_at(data, 32), 1, "format {format}");
        let replacement = u16_at(data, 42);
        assert_eq!(
            face.glyph_bounding_box(ttf_parser::GlyphId(replacement)),
            source.glyph_bounding_box(ttf_parser::GlyphId(target))
        );
    }
    let deleted = subset(&mort(&lookup6(&[(a, 65535)]), 4)).unwrap();
    let face = Face::parse(&deleted, 0).unwrap();
    assert_eq!(
        u16_at(face.raw_face().table(Tag::from_bytes(b"mort")).unwrap(), 42),
        65535
    );
}

#[test]
fn malformed_and_unsupported_mort_does_not_silently_disappear() {
    let source = Face::parse(BASE, 0).unwrap();
    let a = source.glyph_index('a').unwrap().0;
    let data = mort(&lookup6(&[(a, source.glyph_index('Ж').unwrap().0)]), 4);
    for end in 1..data.len() {
        assert!(
            subset(&data[..end]).is_err(),
            "accepted truncated mort at {end}"
        );
    }
    assert!(
        subset(&mort(&lookup6(&[(a, 0)]), 1)).is_err(),
        "unsupported contextual state machine"
    );
    assert!(subset(&mort(&lookup6(&[(a, source.number_of_glyphs())]), 4)).is_err());
}
