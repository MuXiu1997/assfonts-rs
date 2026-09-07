mod common;
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
