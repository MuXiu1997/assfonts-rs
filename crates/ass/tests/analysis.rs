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
        r"{\fn@Vertical}X",
    ] {
        assert!(AssCodec.analyze(&script(text)).is_err(), "{text}");
    }
    assert!(AssCodec
        .analyze(&script("X").replace("Dialogue: 0,", "Dialogue: broken,"))
        .is_err());
    assert!(AssCodec.analyze(&(script("X") + "[Fonts]\n")).is_err());
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
