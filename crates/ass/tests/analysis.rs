use assfonts_ass::AssCodec;
use assfonts_core::{Attachment, FontRequest, SubtitleCodec};

fn script(text: &str) -> String {
    format!("[Script Info]\nScriptType: v4.00+\nWrapStyle: 0\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,First,30,&HFFFFFF,&HFFFFFF,&H0,&H0,0,0,0,0,100,100,0,0,1,0,0,2,0,0,0,1\nStyle: Other,Second,30,&HFFFFFF,&HFFFFFF,&H0,&H0,-1,-1,0,0,100,100,0,0,1,0,0,2,0,0,0,1\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:02.00,Default,,0,0,0,,{text}\n")
}

#[test]
fn ascii_font_names_resets_and_empty_tags_follow_active_style() {
    let usage = AssCodec
        .analyze(&script(
            r"A{\fnArial\b700}B{\rOther}C{\fnOverride}D{\fn\b\i}E{\r}F",
        ))
        .unwrap();
    for (family, weight, italic, characters) in [
        ("First", 400, false, "AF"),
        ("Arial", 700, false, "B"),
        ("Second", 700, true, "CE"),
        ("Override", 700, true, "D"),
    ] {
        assert_eq!(
            usage[&FontRequest {
                family: family.into(),
                weight,
                italic
            }],
            characters.chars().collect()
        );
    }
    assert_eq!(usage.len(), 4);
}

#[test]
fn drawing_soft_breaks_and_visual_transforms() {
    let usage = AssCodec.analyze(&script(r"{\p1}m 0 0 l 50 50{\rOther}l 1 1{\p0\q2}A\nB{\q0}C\nD\hE{\t(0,500,\fs40\alpha&H00&)}F")).unwrap();
    let chars: String = usage.values().flat_map(|c| c.iter()).collect();
    assert_eq!(chars, " ABCDEF\u{a0}");
}

#[test]
fn unsupported_or_malformed_input_does_not_succeed() {
    for text in [
        r"{\t(\b700)}X",
        r"{\fe128}X",
        r"{\unknown}X",
        r"{\rMissing}X",
        "{unclosed",
        r"{\fn@}X",
    ] {
        assert!(AssCodec.analyze(&script(text)).is_err(), "{text}");
    }
    assert!(AssCodec
        .analyze(&script("X").replace("Dialogue: 0,", "Dialogue: broken,"))
        .is_err());
    assert!(AssCodec.analyze(&(script("X") + "[Fonts]\n")).is_err());
}

#[test]
fn vertical_styles_and_overrides_share_physical_fonts_and_keep_layout_tags() {
    let horizontal = script(r"A{\fnSecond}B{\rOther}C{\fnFirst}D{\fn}E{\r}F");
    let vertical = script(r"A{\fn@Second}B{\rOther}C{\fn@First}D{\fn}E{\r}F")
        .replace("Default,First,", "Default,@First,")
        .replace("Other,Second,", "Other,@Second,");
    assert_eq!(
        AssCodec.analyze(&vertical).unwrap(),
        AssCodec.analyze(&horizontal).unwrap()
    );
    let mixed = script(r"A{\fn@First}B{\fnFirst}C");
    assert_eq!(
        AssCodec.analyze(&mixed).unwrap(),
        AssCodec.analyze(&script("ABC")).unwrap()
    );
    let output = AssCodec
        .embed(
            &vertical,
            &[Attachment {
                name: "test_0.ttf".into(),
                data: b"Cat".to_vec(),
            }],
        )
        .unwrap();
    assert_eq!(
        output.replace("[Fonts]\nfontname: test_0.ttf\n1W&U\n\n", ""),
        vertical
    );
    assert!(AssCodec
        .analyze(&script("X").replace("Default,First,", "Default,@,"))
        .is_err());
}

#[test]
fn embedding_preserves_bom_crlf_and_event_text() {
    let original = format!(
        "\u{feff}{}",
        script("猫,{{literal comment}}ABC").replace('\n', "\r\n")
    );
    AssCodec.analyze(&original).unwrap();
    let out = AssCodec
        .embed(
            &original,
            &[Attachment {
                name: "test_0.ttf".into(),
                data: b"Cat".to_vec(),
            }],
        )
        .unwrap();
    let inserted = "[Fonts]\r\nfontname: test_0.ttf\r\n1W&U\r\n\r\n";
    assert_eq!(out.replace(inserted, ""), original);
    assert!(AssCodec
        .embed(
            &original,
            &[Attachment {
                name: "bad\nname".into(),
                data: vec![1]
            }]
        )
        .is_err());
}

#[test]
fn aegisub_metadata_is_ignored_for_analysis_and_preserved_on_output() {
    let plain = script("ABC");
    let metadata = "[Aegisub Project Garbage]\nVideo File: 片源.mkv\nWrapStyle: broken\nStyle: not a real style\nDialogue: not a real event\n";
    for (marker, content) in [
        ("[V4+ Styles]", metadata),
        ("[Events]", "[aegisub project garbage]\n\n"),
        ("[Events]", "[Aegisub Project]\nLast Style: Default\n"),
    ] {
        let input = format!(
            "\u{feff}{}[Aegisub Extradata]\r\nData: 0,key,value",
            plain
                .replace(marker, &format!("{content}{marker}"))
                .replace('\n', "\r\n")
        );
        assert_eq!(
            AssCodec.analyze(&input).unwrap(),
            AssCodec.analyze(&plain).unwrap()
        );
        let output = AssCodec
            .embed(
                &input,
                &[Attachment {
                    name: "test_0.ttf".into(),
                    data: b"Cat".to_vec(),
                }],
            )
            .unwrap();
        assert_eq!(
            output.replace("[Fonts]\r\nfontname: test_0.ttf\r\n1W&U\r\n\r\n", ""),
            input
        );
    }
}

#[test]
fn metadata_support_does_not_suppress_other_parser_diagnostics() {
    let input = script("ABC").replace(
        "[Events]",
        "[Aegisub Project Garbage]\nVideo File: test.mkv\n[Events]",
    );
    assert!(AssCodec
        .analyze(&input.replace("Dialogue: 0,", "Dialogue: bad,"))
        .is_err());
    assert!(AssCodec
        .analyze(&input.replace("[Aegisub Project Garbage]", "[Unknown Section]"))
        .is_err());
    assert!(AssCodec
        .analyze(&input.replace("[Events]", "[Evnts]"))
        .is_err());
}
