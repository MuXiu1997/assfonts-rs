use assfonts_core::{FontFace, Subsetter};
use assfonts_harfbuzz::HarfBuzz;
use std::{collections::BTreeMap, sync::Arc};
use ttf_parser::{Face, Tag};

mod common;
use common::{put16, put32, u16_at, with_table, BASE};

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
            data: Arc::from(with_table(*b"mort", mort)),
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
