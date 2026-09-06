use assfonts_core::{FontFace, Subsetter};
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
