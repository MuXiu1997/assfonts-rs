use assfonts_core::{FontRequest, FontResolver};
use assfonts_fonts::FontCatalog;

const FONT: &[u8] = include_bytes!("../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf");

#[test]
fn matches_aliases_deduplicates_bytes_and_reports_missing_glyphs() {
    let mut catalog = FontCatalog::default();
    catalog.add("original", FONT).unwrap();
    catalog.add("duplicate", FONT).unwrap();
    assert_eq!(catalog.face_count(), 1);
    let request = FontRequest {
        family: "Open Sans".into(),
        weight: 400,
        italic: false,
    };
    assert_eq!(
        catalog.resolve(&request, &['a'].into()).unwrap().source,
        "original"
    );
    assert!(catalog.resolve(&request, &['\u{10ffff}'].into()).is_err());
    assert!(catalog
        .resolve(
            &FontRequest {
                family: "absent".into(),
                ..request
            },
            &['a'].into()
        )
        .is_err());
    assert!(catalog.add("invalid", &b"bad font"[..]).is_err());
    assert_eq!(catalog.face_count(), 1);
}

#[test]
fn equal_ranked_fonts_follow_explicit_catalog_order_like_libass() {
    let mut catalog = FontCatalog::default();
    catalog.add("first", FONT).unwrap();
    // Trailing bytes do not change SFNT tables; simulate a distinct file version.
    let mut another = FONT.to_vec();
    another.push(0);
    catalog.add("second", another).unwrap();
    let request = FontRequest {
        family: "Open Sans".into(),
        weight: 400,
        italic: false,
    };
    assert_eq!(
        catalog.resolve(&request, &['a'].into()).unwrap().source,
        "first"
    );
    let mut reversed = FontCatalog::default();
    let mut another = FONT.to_vec();
    another.push(0);
    reversed.add("second", another).unwrap();
    reversed.add("first", FONT).unwrap();
    assert_eq!(
        reversed.resolve(&request, &['a'].into()).unwrap().source,
        "second"
    );
}

#[test]
fn ties_between_different_coverage_do_not_silently_enable_glyph_fallback() {
    const AC: &[u8] =
        include_bytes!("../../../vendor/harfbuzz/test/api/fonts/Roboto-Regular.ac.ttf");
    const ABC: &[u8] =
        include_bytes!("../../../vendor/harfbuzz/test/api/fonts/Roboto-Regular.abc.ttf");
    let mut catalog = FontCatalog::default();
    catalog.add("z-first-ac", AC).unwrap();
    catalog.add("a-later-abc", ABC).unwrap();
    let request = FontRequest {
        family: "Roboto".into(),
        weight: 400,
        italic: false,
    };
    assert_eq!(
        catalog.resolve(&request, &['a'].into()).unwrap().source,
        "z-first-ac"
    );
    assert!(catalog
        .resolve(&request, &['b'].into())
        .unwrap_err()
        .to_string()
        .contains("missing glyphs"));
    let mut reversed = FontCatalog::default();
    reversed.add("a-later-abc", ABC).unwrap();
    reversed.add("z-first-ac", AC).unwrap();
    assert_eq!(
        reversed.resolve(&request, &['b'].into()).unwrap().source,
        "a-later-abc"
    );
}

#[test]
fn warn_preserves_candidates_order_and_distinguishes_collective_coverage() {
    use assfonts_core::MissingGlyphPolicy;
    let mut catalog = FontCatalog::default();
    catalog
        .add(
            "z-first",
            &include_bytes!("../../../vendor/harfbuzz/test/api/fonts/Roboto-Regular.ac.ttf")[..],
        )
        .unwrap();
    catalog
        .add(
            "a-second",
            &include_bytes!("../../../vendor/harfbuzz/test/api/fonts/Roboto-Regular.abc.ttf")[..],
        )
        .unwrap();
    let request = FontRequest {
        family: "Roboto".into(),
        weight: 400,
        italic: false,
    };
    let usage = [(request, ['b', '\u{378}'].into())].into();
    assert!(catalog.plan(&usage, MissingGlyphPolicy::Error).is_err());
    let plan = catalog.plan(&usage, MissingGlyphPolicy::Warn).unwrap();
    assert_eq!(
        plan.fonts
            .iter()
            .map(|f| f.face.source.as_str())
            .collect::<Vec<_>>(),
        ["z-first", "a-second"]
    );
    assert_eq!(
        plan.fonts.iter().map(|f| f.order).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(plan.warnings.len(), 2);
    assert_eq!(plan.warnings[0].characters, "b\u{378}");
    assert_eq!(plan.warnings[0].missing_from_all_candidates, "\u{378}");
    assert_eq!(plan.warnings[1].characters, "\u{378}");
}
