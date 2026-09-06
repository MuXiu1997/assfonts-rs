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
fn equal_ranked_different_fonts_are_not_chosen_arbitrarily() {
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
    assert!(catalog
        .resolve(&request, &['a'].into())
        .unwrap_err()
        .to_string()
        .contains("ambiguous"));
}
