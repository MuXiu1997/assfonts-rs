mod common;
#[allow(dead_code)]
#[path = "../../fonts/tests/fixtures/legacy.rs"]
mod fixture;
use assfonts_core::{FontFace, FontRequest, FontResolver, MissingGlyphPolicy, Subsetter};
use assfonts_fonts::FontCatalog;
use assfonts_harfbuzz::HarfBuzz;
use std::sync::Arc;

#[test]
fn legacy_only_cmap_is_not_downgraded_to_missing_glyph_warnings() {
    // Valid format-0 cmap labeled Microsoft PRC. A renderer can transcode
    // Unicode into a legacy charmap; our Unicode-only plan cannot preserve it.
    let mut cmap = Vec::new();
    for n in [0, 1, 3, 3] {
        common::put16(&mut cmap, n);
    }
    common::put32(&mut cmap, 12);
    for n in [0, 262, 0] {
        common::put16(&mut cmap, n);
    }
    cmap.resize(274, 0);
    cmap[18 + b'a' as usize] = 1;
    let data = common::with_table(*b"cmap", &cmap);
    let face = FontFace {
        source: "legacy".into(),
        data: Arc::from(data.clone()),
        index: 0,
    };
    let error = HarfBuzz::default()
        .subset_with_policy(&face, &['a'].into(), MissingGlyphPolicy::Warn)
        .unwrap_err();
    assert!(error.to_string().contains("Unicode cmap"));
    let mut catalog = FontCatalog::default();
    catalog.add("legacy", data).unwrap();
    let request = FontRequest {
        family: "Open Sans".into(),
        weight: 400,
        italic: false,
    };
    for chars in [['a'].into(), Default::default()] {
        let result = catalog.plan(&[(request.clone(), chars)].into(), MissingGlyphPolicy::Warn);
        assert!(matches!(result, Err(assfonts_core::Error::Unsupported(_))));
    }
}

#[test]
fn prc_subset_keeps_outlines_metrics_aliases_and_notdef() {
    let original = fixture::font();
    let before = original.clone();
    let face = FontFace {
        source: "original-prc.ttf".into(),
        data: Arc::from(original),
        index: 0,
    };
    let hb = HarfBuzz::default();
    let characters = ['中', '「', '」', '€', '\u{e7c7}', '\u{f8f5}'].into();
    let output = hb
        .subset_with_policy(&face, &characters, MissingGlyphPolicy::Error)
        .unwrap();
    let source = ttf_parser::Face::parse(fixture::ORIGINAL, 0).unwrap();
    let subset = ttf_parser::Face::parse(&output, 0).unwrap();
    assert!(subset.number_of_glyphs() < source.number_of_glyphs());
    for (ch, original_ch) in [
        ('中', 'B'),
        ('「', 'A'),
        ('」', 'A'),
        ('€', 'C'),
        ('\u{e7c7}', 'D'),
        ('\u{f8f5}', 'F'),
    ] {
        let a = source.glyph_index(original_ch).unwrap();
        let b = subset.glyph_index(ch).unwrap();
        assert_eq!(source.glyph_bounding_box(a), subset.glyph_bounding_box(b));
        assert_eq!(source.glyph_hor_advance(a), subset.glyph_hor_advance(b));
        assert_eq!(
            source.glyph_hor_side_bearing(a),
            subset.glyph_hor_side_bearing(b)
        );
    }
    assert_eq!(subset.glyph_index('「'), subset.glyph_index('」'));
    assert_eq!(&*face.data, &before);
    let absent = ['中', '\u{378}'].into();
    assert!(hb
        .subset_with_policy(&face, &absent, MissingGlyphPolicy::Error)
        .is_err());
    let warning = hb
        .subset_with_policy(&face, &absent, MissingGlyphPolicy::Warn)
        .unwrap();
    let warning = ttf_parser::Face::parse(&warning, 0).unwrap();
    assert!(warning.glyph_index('中').is_some());
    assert!(warning.glyph_index('\u{378}').is_none());
    assert_eq!(
        source.glyph_bounding_box(ttf_parser::GlyphId(0)),
        warning.glyph_bounding_box(ttf_parser::GlyphId(0))
    );
}

#[test]
fn prc_collection_extracts_the_requested_face_before_subsetting() {
    let original = fixture::font();
    let collection = fixture::collection(&[fixture::ORIGINAL, &original]);
    let standalone = FontFace {
        source: "standalone".into(),
        data: Arc::from(original),
        index: 0,
    };
    let collection = FontFace {
        source: "collection".into(),
        data: Arc::from(collection),
        index: 1,
    };
    let hb = HarfBuzz::default();
    assert_eq!(
        hb.subset(&standalone, &['中'].into()).unwrap(),
        hb.subset(&collection, &['中'].into()).unwrap()
    );
}
