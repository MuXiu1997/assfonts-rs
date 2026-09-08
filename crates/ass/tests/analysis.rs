use assfonts_ass::{AssCodec, ParseMode};
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
fn invalid_low_bold_values_fall_back_to_active_style_only() {
    for value in [-100, -1, 2, 4, 20, 99] {
        let actual = script(&format!(
            r"{{\b1\b{value}}}A{{\rOther\b0\fnOverride\i0\b{value}}}B{{\rMissing\b1\b{value}}}C"
        ));
        let expected = script(r"{\b0}A{\rOther\fnOverride\i0\b1}B{\rMissing\b0}C");
        assert_eq!(
            AssCodec.analyze(&actual).unwrap(),
            AssCodec.analyze(&expected).unwrap(),
            "{value}"
        );
    }
}

#[test]
fn valid_bold_values_and_unsupported_high_values_keep_their_contract() {
    for (value, weight) in [
        (0, 400),
        (1, 700),
        (100, 100),
        (400, 400),
        (700, 700),
        (900, 900),
    ] {
        let usage = AssCodec
            .analyze(&script(&format!(r"{{\b{value}}}A")))
            .unwrap();
        assert_eq!(
            usage[&FontRequest {
                family: "First".into(),
                weight,
                italic: false
            }],
            ['A'].into()
        );
    }
    assert!(AssCodec.analyze(&script(r"{\b901}A")).is_err());
}

#[test]
fn tagless_transforms_do_not_change_font_usage() {
    for transform in [r"\t()", r"\t(100,6590)", r"\t(100,6590"] {
        let actual = script(&format!(r"{{{transform}}}A{{\fnSecond}}B"));
        assert_eq!(
            AssCodec.analyze(&actual).unwrap(),
            AssCodec.analyze(&script(r"A{\fnSecond}B")).unwrap()
        );
    }
}

#[test]
fn unterminated_visual_transforms_keep_the_final_tag_and_next_block() {
    for tags in [r"\c&HFAFAFC&", r"\fscx120\blur2", r"\alpha&HFF&"] {
        let closed = script(&format!(r"{{\t(37,750,{tags})}}A{{\fnSecond}}B"));
        let open = closed.replace(&format!("{tags})"), tags);
        assert_eq!(
            AssCodec.analyze(&open).unwrap(),
            AssCodec.analyze(&closed).unwrap()
        );
    }
    for tags in [
        r"\fnSecond",
        r"\b1",
        r"\i1",
        r"\rOther",
        r"\p1",
        r"\fe128",
        r"\t(\b1)",
    ] {
        for ending in ["", ")"] {
            assert!(
                AssCodec
                    .analyze(&script(&format!(r"{{\t(37,750,{tags}{ending}}}A")))
                    .is_err(),
                "{tags}{ending}"
            );
        }
    }
}

#[test]
fn drawing_soft_breaks_and_visual_transforms() {
    let usage = AssCodec.analyze(&script(r"{\p1}m 0 0 l 50 50{\rOther}l 1 1{\p0\q2}A\nB{\q0}C\nD\hE{\t(0,500,\fs40\alpha&H00&)}F")).unwrap();
    let chars: String = usage.values().flat_map(|c| c.iter()).collect();
    assert_eq!(chars, " ABCDEF\u{a0}");
}

#[test]
fn drawing_runs_keep_fonts_weights_and_resets_without_path_characters() {
    let text = r"{\p1}m 0 0 l 50 50{\fnMask\b700}m 1 1 l 2 2{\rOther}m 3 3 l 4 4{\p0}X";
    let usage = AssCodec.analyze(&script(text)).unwrap();
    assert!(usage[&FontRequest {
        family: "First".into(),
        weight: 400,
        italic: false
    }]
        .is_empty());
    assert!(usage[&FontRequest {
        family: "Mask".into(),
        weight: 700,
        italic: false
    }]
        .is_empty());
    assert_eq!(
        usage[&FontRequest {
            family: "Second".into(),
            weight: 700,
            italic: true
        }],
        ['X'].into()
    );
    assert_eq!(usage.len(), 3);
}

#[test]
fn unsupported_or_malformed_input_does_not_succeed() {
    for text in [
        r"{\t(\b700)}X",
        r"{\fe128}X",
        r"{\fe128}X",
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

#[test]
fn dialogue_style_lookup_matches_libass_stars_default_and_last_definition() {
    let expected = AssCodec.analyze(&script("X")).unwrap();
    for name in ["*Default", "**dEfAuLt", "Missing"] {
        let input = script("X").replace("0:00:02.00,Default,", &format!("0:00:02.00,{name},"));
        assert_eq!(AssCodec.analyze(&input).unwrap(), expected, "{name}");
    }
    let duplicate = script("X").replace("Style: Other,", "Style: Default,");
    assert_eq!(
        AssCodec.analyze(&duplicate).unwrap(),
        AssCodec.analyze(&script(r"{\rOther}X")).unwrap()
    );
    let starred = script("X").replace("Style: Other,", "Style: **Other,");
    // ass-core interprets stars on Style definitions as its inheritance
    // extension. This change does not suppress that parser diagnostic.
    assert!(AssCodec.analyze(&starred).is_err());
}

#[test]
fn missing_reset_uses_original_dialogue_style_not_global_default() {
    let expected = AssCodec.analyze(&script(r"{\rOther}X")).unwrap();
    for reset in ["0", "Missing", "*Default", "other"] {
        let input = script(&format!(r"{{\rDefault\r{reset}}}X"))
            .replace("0:00:02.00,Default,", "0:00:02.00,Other,");
        assert_eq!(AssCodec.analyze(&input).unwrap(), expected, "{reset}");
    }
    let named_zero = script(r"{\r0}X").replace("Style: Other,", "Style: 0,");
    assert_eq!(AssCodec.analyze(&named_zero).unwrap(), expected);
}

#[test]
fn missing_dialogue_style_uses_libass_implicit_default_without_a_named_default() {
    let input = script("X").replace("Style: Default,First,", "Style: Custom,First,");
    let usage = AssCodec.analyze(&input).unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(
        usage[&FontRequest {
            family: "Arial".into(),
            weight: 200,
            italic: false
        }],
        ['X'].into()
    );
}

#[test]
fn recoverable_empty_overrides_do_not_lose_font_or_drawing_state() {
    for (original, normalized) in [
        (r"{\\fnArial\b1}X", r"{\fnArial\b1}X"),
        (r"{\b1\\rOther}X", r"{\rOther}X"),
        (r"{\fnArial\}X", r"{\fnArial}X"),
        (r"{\\p1}m 0 0 l 10 10{\p0}X", r"{\p1}m 0 0 l 10 10{\p0}X"),
        (r"{\t(0,100,\\fs40)}X", r"{\t(0,100,\fs40)}X"),
    ] {
        assert_eq!(
            AssCodec.analyze(&script(original)).unwrap(),
            AssCodec.analyze(&script(normalized)).unwrap()
        );
    }
    for input in [r"{\\fe128}X", r"{\t(\\fnArial)}X"] {
        assert!(AssCodec.analyze(&script(input)).is_err(), "{input}");
    }
    for (original, normalized) in [(r"{\字}X", "X"), (r"{\ fnArial}X", r"{\fnArial}X")] {
        assert!(AssCodec.analyze(&script(original)).is_err());
        assert_eq!(
            AssCodec::with_mode(ParseMode::Compatible)
                .analyze(&script(original))
                .unwrap(),
            AssCodec.analyze(&script(normalized)).unwrap()
        );
    }
}

#[test]
fn color_argument_boundaries_preserve_usage_and_original_subtitle() {
    let input = script(r"{\alphaFF\3cFFFFFF\1cA0\1alhpa\ccccccc\fnArial}X{\alpha00}Y");
    assert_eq!(
        AssCodec.analyze(&input).unwrap(),
        AssCodec.analyze(&script(r"{\fnArial}XY")).unwrap()
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
        output.replace("[Fonts]\nfontname: test_0.ttf\n1W&U\n\n", ""),
        input
    );
}
