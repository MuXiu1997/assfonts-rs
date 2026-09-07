#[path = "../../harfbuzz/tests/common/mod.rs"]
mod common;
#[path = "fixtures/legacy.rs"]
mod fixture;
use assfonts_core::{FontRequest, FontResolver, MissingGlyphPolicy};
use assfonts_fonts::{missing_characters, prepare_unicode_face, verify_coverage, FontCatalog};
use std::collections::BTreeSet;
use ttf_parser::{Face, Tag};

fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[test]
fn normalization_preserves_source_glyph_ids_tables_and_original_input() {
    let data = fixture::font();
    let before = data.clone();
    let output = prepare_unicode_face(&data, 0).unwrap().unwrap();
    assert_eq!(data, before);
    let original = Face::parse(fixture::ORIGINAL, 0).unwrap();
    let source = Face::parse(&data, 0).unwrap();
    let normalized = Face::parse(&output, 0).unwrap();
    assert_eq!(source.number_of_glyphs(), normalized.number_of_glyphs());
    for (ch, glyph_char) in [
        ('A', 'A'),
        ('中', 'B'),
        ('「', 'A'),
        ('」', 'A'),
        ('€', 'C'),
        ('\u{e7c7}', 'D'),
        ('\u{f8f5}', 'F'),
    ] {
        let glyph = original.glyph_index(glyph_char).unwrap();
        assert_eq!(normalized.glyph_index(ch), Some(glyph));
        assert_eq!(
            source.glyph_bounding_box(glyph),
            normalized.glyph_bounding_box(glyph)
        );
        assert_eq!(
            source.glyph_hor_advance(glyph),
            normalized.glyph_hor_advance(glyph)
        );
    }
    let absent: BTreeSet<_> = ['丂', '肿', '\u{378}', '\u{1e3f}', '😀'].into();
    assert_eq!(missing_characters(&data, 0, &absent).unwrap(), absent);
    for ch in &absent {
        assert!(normalized.glyph_index(*ch).is_none());
    }
    let raw = source.raw_face();
    for record in raw.table_records {
        let old = raw.table(record.tag).unwrap();
        let new = normalized.raw_face().table(record.tag).unwrap();
        if record.tag == Tag::from_bytes(b"cmap") {
            continue;
        }
        if record.tag == Tag::from_bytes(b"head") {
            assert_eq!(&old[..8], &new[..8]);
            assert_eq!(&old[12..], &new[12..]);
        } else {
            assert_eq!(old, new, "table {:?} changed", record.tag);
        }
    }
    let old_cmap = raw.table(Tag::from_bytes(b"cmap")).unwrap();
    let new_cmap = normalized
        .raw_face()
        .table(Tag::from_bytes(b"cmap"))
        .unwrap();
    assert_eq!(&old_cmap[12..], &new_cmap[20..20 + old_cmap.len() - 12]);
    assert_eq!(checksum(&output), 0xb1b0afba);
    assert!(prepare_unicode_face(&output, 0).unwrap().is_none());
}

#[test]
fn unicode_faces_stay_byte_identical_and_ttc_face_identity_is_retained() {
    assert!(prepare_unicode_face(fixture::ORIGINAL, 0)
        .unwrap()
        .is_none());
    let legacy = fixture::font();
    let ttc = fixture::collection(&[fixture::ORIGINAL, &legacy]);
    let copy = ttc.clone();
    assert!(prepare_unicode_face(&ttc, 0).unwrap().is_none());
    let output = prepare_unicode_face(&ttc, 1).unwrap().unwrap();
    assert_eq!(ttc, copy);
    assert_eq!(ttf_parser::fonts_in_collection(&output), None);
    let expected = Face::parse(fixture::ORIGINAL, 0).unwrap().glyph_index('B');
    assert_eq!(Face::parse(&output, 0).unwrap().glyph_index('中'), expected);
    assert!(prepare_unicode_face(&ttc, 2).is_err());
    let mut catalog = FontCatalog::default();
    catalog.add("original.ttc", ttc.clone()).unwrap();
    let request = FontRequest {
        family: "Open Sans".into(),
        weight: 400,
        italic: false,
    };
    let plan = catalog
        .plan(&[(request, ['中'].into())].into(), MissingGlyphPolicy::Warn)
        .unwrap();
    assert_eq!(plan.fonts.len(), 2);
    assert_eq!(plan.fonts[1].face.index, 1);
    assert_eq!(&*plan.fonts[1].face.data, &ttc);
    assert_eq!(plan.warnings.len(), 1); // Unicode Open Sans lacks 中; GBK face has it.
    assert_eq!(plan.warnings[0].face_index, 0);
    assert!(plan.warnings[0].missing_from_all_candidates.is_empty());
}

#[test]
fn coverage_and_planning_use_actual_prc_glyphs_in_both_policies() {
    let data = fixture::font();
    let chars = ['A', '中', '「', '」', '€', '\u{e7c7}', '\u{f8f5}'].into();
    verify_coverage(&data, 0, &chars).unwrap();
    assert!(missing_characters(&data, 0, &chars).unwrap().is_empty());
    let mut catalog = FontCatalog::default();
    catalog.add("source", data.clone()).unwrap();
    let request = FontRequest {
        family: "Open Sans".into(),
        weight: 400,
        italic: false,
    };
    for policy in [MissingGlyphPolicy::Error, MissingGlyphPolicy::Warn] {
        let plan = catalog
            .plan(&[(request.clone(), chars.clone())].into(), policy)
            .unwrap();
        assert!(plan.warnings.is_empty());
        assert_eq!(&*plan.fonts[0].face.data, &data);
    }
    let missing: BTreeSet<_> = ['\u{378}', '中'].into();
    assert!(catalog
        .plan(
            &[(request.clone(), missing.clone())].into(),
            MissingGlyphPolicy::Error
        )
        .is_err());
    let plan = catalog
        .plan(&[(request, missing)].into(), MissingGlyphPolicy::Warn)
        .unwrap();
    assert_eq!(plan.warnings[0].characters, "\u{378}");
    assert_eq!(plan.warnings[0].missing_from_all_candidates, "\u{378}");
}

#[test]
fn corrupt_or_unselected_legacy_charmaps_do_not_silently_convert() {
    let valid = fixture::cmap(&[(0x41, 1), (0xd6d0, 2)]);
    for (at, value) in [
        (12 + 6, 1u16),
        (12 + 518 + 2, 300),
        (12 + 518 + 6, 0),
        (12 + 2, 525),
    ] {
        let mut cmap = valid.clone();
        cmap[at..at + 2].copy_from_slice(&value.to_be_bytes());
        let data = fixture::with_cmap(&cmap);
        assert!(
            prepare_unicode_face(&data, 0).is_err(),
            "accepted corrupt field at {at}"
        );
        assert!(missing_characters(&data, 0, &['A'].into()).is_err());
    }
    // Big5 is not CP936. An earlier Microsoft symbol cmap takes priority;
    // do not find and convert a later PRC record behind the renderer's back.
    let mut big5 = valid.clone();
    big5[6..8].copy_from_slice(&4u16.to_be_bytes());
    assert!(prepare_unicode_face(&fixture::with_cmap(&big5), 0).is_err());
    let mut mixed = vec![0, 0, 0, 2, 0, 3, 0, 0, 0, 0, 0, 20, 0, 3, 0, 3, 0, 0, 0, 20];
    mixed.extend_from_slice(&valid[12..]);
    assert!(prepare_unicode_face(&fixture::with_cmap(&mixed), 0).is_err());
}

#[test]
fn format2_delta_wraps_but_zero_entries_remain_missing() {
    for (raw, delta, expected) in [(3u16, 0xffffu16, 2u16), (0xffff, 2, 1), (0xffff, 1, 0)] {
        let mut cmap = fixture::cmap(&[(0xd6d0, raw), (0xd6d1, 0)]);
        // The first non-ASCII subheader is index one. Both glyphs share it.
        let at = 12 + 518 + 8 + 4;
        cmap[at..at + 2].copy_from_slice(&delta.to_be_bytes());
        let data = fixture::with_cmap(&cmap);
        let output = prepare_unicode_face(&data, 0).unwrap().unwrap();
        let face = Face::parse(&output, 0).unwrap();
        assert_eq!(
            face.glyph_index('中'),
            (expected != 0).then_some(ttf_parser::GlyphId(expected))
        );
        assert!(
            face.glyph_index('肿').is_none(),
            "applied delta to a missing zero glyph"
        );
    }
}
