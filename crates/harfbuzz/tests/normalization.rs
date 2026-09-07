mod common;
use assfonts_core::{FontFace, MissingGlyphPolicy, Subsetter};
use assfonts_harfbuzz::HarfBuzz;
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf");

#[test]
fn canonical_forms_survive_subsetting_in_both_directions() {
    let face = FontFace {
        source: "OpenSans-Regular.ttf".into(),
        data: Arc::from(FONT),
        index: 0,
    };
    for requested in [vec!['a', '\u{301}'], vec!['á']] {
        let output = HarfBuzz::default()
            .subset(&face, &requested.into_iter().collect())
            .unwrap();
        let font = ttf_parser::Face::parse(&output, 0).unwrap();
        for ch in ['a', '\u{301}', 'á'] {
            assert!(
                font.glyph_index(ch).is_some(),
                "missing canonical form {ch}"
            );
        }
        assert!(
            font.glyph_index('Ж').is_none(),
            "unrelated character retained"
        );
    }
}

#[test]
fn warning_keeps_decomposition_even_when_literal_character_has_no_cmap_entry() {
    let original = ttf_parser::Face::parse(FONT, 0).unwrap();
    let mut cmap = Vec::new();
    for value in [0, 1, 3, 10] {
        common::put16(&mut cmap, value);
    }
    common::put32(&mut cmap, 12);
    for value in [12, 0] {
        common::put16(&mut cmap, value);
    }
    for value in [40, 0, 2] {
        common::put32(&mut cmap, value);
    }
    for ch in ['a', '\u{301}'] {
        for value in [
            ch as u32,
            ch as u32,
            original.glyph_index(ch).unwrap().0 as u32,
        ] {
            common::put32(&mut cmap, value);
        }
    }
    let face = FontFace {
        source: "decomposed-only".into(),
        data: Arc::from(common::with_table(*b"cmap", &cmap)),
        index: 0,
    };
    let requested = ['á'].into();
    let hb = HarfBuzz::default();
    assert!(hb
        .subset_with_policy(&face, &requested, MissingGlyphPolicy::Error)
        .is_err());
    let output = hb
        .subset_with_policy(&face, &requested, MissingGlyphPolicy::Warn)
        .unwrap();
    let parsed = ttf_parser::Face::parse(&output, 0).unwrap();
    assert!(parsed.glyph_index('á').is_none());
    for ch in ['a', '\u{301}'] {
        assert!(parsed.glyph_index(ch).is_some());
    }
}
