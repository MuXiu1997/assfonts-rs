use assfonts_core::{FontFace, Subsetter};
use assfonts_harfbuzz::HarfBuzz;
use std::{collections::BTreeSet, sync::Arc};

#[test]
fn subsets_glyf_and_cff_and_rejects_invalid_inputs() {
    for (source, data, chars, cff) in [
        (
            "Open Sans",
            &include_bytes!("../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf")[..],
            "abc",
            false,
        ),
        (
            "Source Sans",
            &include_bytes!("../../../vendor/harfbuzz/test/api/fonts/SourceSansPro-Regular.ac.otf")
                [..],
            "a",
            true,
        ),
    ] {
        let face = FontFace {
            source: source.into(),
            data: Arc::from(data),
            index: 0,
        };
        let chars: BTreeSet<char> = chars.chars().collect();
        let output = HarfBuzz::default().subset(&face, &chars).unwrap();
        let parsed = ttf_parser::Face::parse(&output, 0).unwrap();
        assert_eq!(parsed.tables().cff.is_some(), cff);
        assert!(
            parsed.number_of_glyphs()
                < ttf_parser::Face::parse(data, 0).unwrap().number_of_glyphs()
        );
        assert_eq!(HarfBuzz::default().subset(&face, &chars).unwrap(), output);
        assert!(HarfBuzz::default()
            .subset(
                &FontFace {
                    index: 999,
                    ..face.clone()
                },
                &chars
            )
            .is_err());
        assert!(HarfBuzz::default()
            .subset(&face, &['\u{10ffff}'].into())
            .is_err());
    }
    assert!(HarfBuzz::default()
        .subset(
            &FontFace {
                source: "bad".into(),
                data: Arc::from(&b"invalid"[..]),
                index: 0
            },
            &['a'].into()
        )
        .is_err());
}

#[test]
fn extracts_nonzero_collection_face() {
    let data =
        &include_bytes!("../../../vendor/harfbuzz/test/shape/data/in-house/fonts/TTC.ttc")[..];
    assert!(ttf_parser::fonts_in_collection(data).unwrap() > 1);
    let original = ttf_parser::Face::parse(data, 1).unwrap();
    let mut chars = BTreeSet::new();
    for subtable in original.tables().cmap.unwrap().subtables {
        if subtable.is_unicode() {
            subtable.codepoints(|n| {
                if let Some(c) = char::from_u32(n) {
                    if original.glyph_index(c).is_some_and(|g| g.0 != 0) {
                        chars.insert(c);
                    }
                }
            });
        }
    }
    let chars = chars.into_iter().take(3).collect();
    let face = FontFace {
        source: "TTC".into(),
        data: Arc::from(data),
        index: 1,
    };
    let output = HarfBuzz::default().subset(&face, &chars).unwrap();
    assert!(ttf_parser::fonts_in_collection(&output).is_none());
    assert!(ttf_parser::Face::parse(&output, 0).is_ok());
}
