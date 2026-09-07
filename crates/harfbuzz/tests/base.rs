use assfonts_core::{FontFace, Subsetter};
use assfonts_harfbuzz::HarfBuzz;
use std::sync::Arc;
use ttf_parser::{Face, Tag};

mod common;
use common::{put16, put32, with_table, BASE};

fn baseline(version: u32) -> Vec<u8> {
    let mut b = vec![];
    put32(&mut b, version);
    put16(&mut b, if version == 0x10000 { 8 } else { 12 });
    put16(&mut b, 0); // no vertical axis
    if version != 0x10000 {
        put32(&mut b, 0);
    } // no variation store
    for value in [4, 10, 1] {
        put16(&mut b, value);
    }
    b.extend(b"romn");
    put16(&mut b, 1);
    b.extend(b"DFLT");
    for value in [8, 6, 0, 0, 0, 1, 6, 1, 42] {
        put16(&mut b, value);
    }
    b
}

fn subset(data: Vec<u8>) -> assfonts_core::Result<Vec<u8>> {
    HarfBuzz::default().subset(
        &FontFace {
            source: "synthetic optional table".into(),
            data: Arc::from(data),
            index: 0,
        },
        &['a'].into(),
    )
}

#[test]
fn valid_base_versions_are_not_dropped() {
    for version in [0x10000, 0x10001] {
        let result = subset(with_table(*b"BASE", &baseline(version))).unwrap();
        let face = Face::parse(&result, 0).unwrap();
        let table = face.raw_face().table(Tag::from_bytes(b"BASE")).unwrap();
        assert_eq!(&table[..4], &version.to_be_bytes());
        assert!(table.windows(4).any(|b| b == b"romn"));
        assert!(table.windows(4).any(|b| b == [0, 1, 0, 42]));
    }
}

#[test]
fn malformed_optional_base_matches_absent_base_without_full_font_fallback() {
    let expected = subset(BASE.to_vec()).unwrap();
    let mut wrong_version = baseline(0x10000);
    // Dream Han declares 1.1 over a 1.0 body: the axis bytes become a bogus
    // ItemVariationStore offset. HB rendering rejects that table too.
    wrong_version[3] = 1;
    let mut cases = vec![wrong_version];
    let valid = baseline(0x10001);
    for length in 1..8 {
        cases.push(valid[..length].to_vec());
    }
    for bad in cases {
        let result = subset(with_table(*b"BASE", &bad)).unwrap();
        assert_eq!(result, expected);
        let face = Face::parse(&result, 0).unwrap();
        assert!(face.raw_face().table(Tag::from_bytes(b"BASE")).is_none());
        assert!(face.number_of_glyphs() < Face::parse(BASE, 0).unwrap().number_of_glyphs());
    }
}

#[test]
fn unrelated_malformed_layout_is_not_silently_discarded() {
    assert!(subset(with_table(*b"GPOS", &[0, 1, 0])).is_err());
}
