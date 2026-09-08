use assfonts_ass::AssCodec;
use assfonts_core::{Attachment, FontRequest, SubtitleCodec};

fn script(text: &str) -> String {
    let mut input = include_str!("../../../examples/basic.ass")
        .lines()
        .filter(|line| !line.starts_with("Dialogue:"))
        .collect::<Vec<_>>()
        .join("\n");
    input.push_str(&format!(
        "\nDialogue: 0,0:00:00.00,0:00:02.00,Default,,0,0,0,,{text}\n"
    ));
    input
}

#[test]
fn tabs_collect_spaces_without_losing_font_transitions() {
    let text = "A\tB{\\fnOther}C\tD{\\r}E\tF";
    let usage = AssCodec.analyze(&script(text)).unwrap();
    assert_eq!(
        usage,
        AssCodec.analyze(&script(&text.replace('\t', " "))).unwrap()
    );
    assert_eq!(usage.len(), 2);
    assert!(usage
        .values()
        .all(|chars| chars.contains(&' ') && !chars.contains(&'\t')));
}

#[test]
fn del_remains_a_font_character_and_is_not_silently_removed() {
    let usage = AssCodec
        .analyze(&script("A\u{7f}B{\\fnOther}\u{7f}C"))
        .unwrap();
    assert_eq!(usage.len(), 2);
    for (family, chars) in [("Open Sans", "AB\u{7f}"), ("Other", "C\u{7f}")] {
        assert_eq!(
            usage[&FontRequest {
                family: family.into(),
                weight: 400,
                italic: false,
            }],
            chars.chars().collect()
        );
    }
}

#[test]
fn preamble_is_ignored_not_promoted_to_script_info() {
    let complete = script(r"A\nB");
    let without_info = &complete[complete.find("[V4+ Styles]").unwrap()..];
    let expected = AssCodec.analyze(without_info).unwrap();
    assert!(expected.values().all(|chars| chars.contains(&' ')));
    for ending in ["\n", "\r\n"] {
        let input = format!(
            "\u{feff}; [Script Info]\n==== 未分段的前言 ====\nWrapStyle: 2\nPlayResX: 1\n{without_info}"
        )
        .replace('\n', ending);
        assert_eq!(AssCodec.analyze(&input).unwrap(), expected);
        let bad_section = input.replace("[Events]", "[Unknown]\n[Events]");
        assert!(AssCodec.analyze(&bad_section).is_err());
    }
}

#[test]
fn control_support_does_not_suppress_other_errors_or_rewrite_source() {
    for ch in ['\0', '\u{1}', '\u{b}', '\u{85}'] {
        assert!(AssCodec.analyze(&script(&format!("A{ch}B"))).is_err());
    }
    assert!(AssCodec.analyze(&script("A\t{\\unknown}B")).is_err());
    let input = script("A\tB\u{7f}C");
    let fonts = [Attachment {
        name: "fixture.ttf".into(),
        data: b"Cat".to_vec(),
    }];
    let embedded = AssCodec.embed(&input, &fonts).unwrap();
    assert_eq!(
        embedded.replace("[Fonts]\nfontname: fixture.ttf\n1W&U\n\n", ""),
        input
    );
}
