//! Synthetic, redistributable legacy charmaps over the vendored OFL fixture.
use crate::common as sfnt;
use std::collections::BTreeMap;

pub const ORIGINAL: &[u8] = sfnt::BASE;

pub fn cmap(entries: &[(u16, u16)]) -> Vec<u8> {
    let mut rows: BTreeMap<u16, BTreeMap<u16, u16>> = BTreeMap::new();
    rows.insert(0, BTreeMap::new());
    for &(code, glyph) in entries {
        rows.entry(code >> 8).or_default().insert(code & 255, glyph);
    }
    let mut table = vec![0u8; 518 + rows.len() * 8];
    table[..2].copy_from_slice(&2u16.to_be_bytes());
    for (index, (&lead, row)) in rows.iter().enumerate() {
        if lead != 0 {
            table[6 + lead as usize * 2..8 + lead as usize * 2]
                .copy_from_slice(&((index * 8) as u16).to_be_bytes());
        }
        if row.is_empty() {
            continue;
        }
        let first = *row.keys().next().unwrap();
        let last = *row.keys().next_back().unwrap();
        let at = 518 + index * 8;
        table[at..at + 2].copy_from_slice(&first.to_be_bytes());
        table[at + 2..at + 4].copy_from_slice(&(last - first + 1).to_be_bytes());
        let offset = (table.len() - at - 6) as u16;
        table[at + 6..at + 8].copy_from_slice(&offset.to_be_bytes());
        for low in first..=last {
            sfnt::put16(&mut table, *row.get(&low).unwrap_or(&0));
        }
    }
    let length = u16::try_from(table.len()).unwrap();
    table[2..4].copy_from_slice(&length.to_be_bytes());
    let mut out = Vec::new();
    for value in [0, 1, 3, 3] {
        sfnt::put16(&mut out, value);
    }
    sfnt::put32(&mut out, 12);
    out.extend(table);
    out
}

pub fn with_cmap(cmap: &[u8]) -> Vec<u8> {
    sfnt::with_table(*b"cmap", cmap)
}

pub fn font() -> Vec<u8> {
    let face = ttf_parser::Face::parse(ORIGINAL, 0).unwrap();
    let glyph = |ch| face.glyph_index(ch).unwrap().0;
    with_cmap(&cmap(&[
        (0x20, glyph(' ')),
        (0x40, glyph('@')),
        (0x41, glyph('A')),
        (0x42, glyph('B')),
        (0x80, glyph('C')),
        (0xff, glyph('F')),
        (0xa1b8, glyph('A')),
        (0xa1b9, glyph('A')), // distinct codes, same glyph
        (0xa8bc, glyph('D')),
        (0xd6d0, glyph('B')),
        (0xd6d1, 0),
    ]))
}

pub fn collection(faces: &[&[u8]]) -> Vec<u8> {
    let mut out = b"ttcf".to_vec();
    sfnt::put32(&mut out, 0x10000);
    sfnt::put32(&mut out, faces.len() as u32);
    out.resize(12 + faces.len() * 4, 0);
    for (index, face) in faces.iter().enumerate() {
        out.resize(out.len().next_multiple_of(4), 0);
        let base = out.len();
        out[12 + index * 4..16 + index * 4].copy_from_slice(&(base as u32).to_be_bytes());
        out.extend_from_slice(face);
        for i in 0..sfnt::u16_at(face, 4) as usize {
            let at = base + 12 + i * 16 + 8;
            let offset = u32::from_be_bytes(out[at..at + 4].try_into().unwrap());
            out[at..at + 4].copy_from_slice(&(offset + base as u32).to_be_bytes());
        }
    }
    out
}
