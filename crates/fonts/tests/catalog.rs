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
