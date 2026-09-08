use assfonts_ass::{AssCodec, ParseMode};
use assfonts_core::{FontRequest, SubtitleCodec};

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
fn unknown_and_visual_prefixes_preserve_font_usage() {
    let codec = AssCodec::with_mode(ParseMode::Compatible);
    for tags in [
        r"\m\0\N\ASEDARK\fa\nlur\f",
        r"\border3\shade2\fra20\frzmath.sin(3)\fsc\fsHYQiHei",
        r"\unknown(\fnHidden)\ZZZ(\b1)\fnOpen Sans",
    ] {
        assert_eq!(
            codec.analyze(&script(&format!("{{{tags}}}AB"))).unwrap(),
            AssCodec.analyze(&script("AB")).unwrap(),
            "{tags}"
        );
    }
}

#[test]
fn malformed_bold_prefixes_are_not_silently_ignored() {
    let codec = AssCodec::with_mode(ParseMode::Compatible);
    assert_eq!(
        codec.analyze(&script(r"{\b1\border3\blur2\be1}A")).unwrap(),
        AssCodec.analyze(&script(r"{\b1}A")).unwrap()
    );
    for tag in [r"\blu3", r"\bklur4"] {
        let usage = codec
            .analyze(&script(&format!(r"{{\b1}}A{{{tag}}}B")))
            .unwrap();
        assert_eq!(
            usage[&FontRequest {
                family: "Open Sans".into(),
                weight: 400,
                italic: false
            }],
            ['B'].into()
        );
        assert_eq!(
            usage[&FontRequest {
                family: "Open Sans".into(),
                weight: 700,
                italic: false
            }],
            ['A'].into()
        );
    }
}

#[test]
fn numeric_prefixes_parenthesized_arguments_and_state_resets_match_libass() {
    let codec = AssCodec::with_mode(ParseMode::Compatible);
    for arg in ["(1)", "1.7", "+1tail"] {
        assert_eq!(
            codec.analyze(&script(&format!(r"{{\b{arg}}}A"))).unwrap(),
            AssCodec.analyze(&script(r"{\b1}A")).unwrap()
        );
    }
    assert_eq!(
        codec
            .analyze(&script(r"{\i1\i2\q9\p1}m 0 0 l 1 1{\pOops}A"))
            .unwrap(),
        AssCodec.analyze(&script("A")).unwrap()
    );
    assert_eq!(
        codec.analyze(&script(r"{\fn(Other)}A")).unwrap(),
        AssCodec.analyze(&script(r"{\fnOther}A")).unwrap()
    );
}

#[test]
fn excessive_transform_nesting_is_bounded() {
    let tags = format!("{}\\fs20{}", "\\t(".repeat(66), ")".repeat(66));
    let error = AssCodec
        .analyze(&script(&format!("{{{tags}}}A")))
        .unwrap_err();
    assert!(error.to_string().contains("nesting exceeds 64"));
}

#[test]
fn visual_nested_transforms_and_first_closing_parenthesis_match_libass() {
    let nested = r"{\t(0,1000,\frz5\t(500,1000,\frz0)\t(900,1000,\alpha&HFF&))}X";
    assert_eq!(
        AssCodec.analyze(&script(nested)).unwrap(),
        AssCodec.analyze(&script("X")).unwrap()
    );
    let after = r"{\t(\t(\fs40)\fnSecond}X";
    assert_eq!(
        AssCodec.analyze(&script(after)).unwrap(),
        AssCodec.analyze(&script(r"{\fnSecond}X")).unwrap()
    );
}

#[test]
fn animation_still_rejects_font_state_and_encoding_changes() {
    for tags in [
        r"\fnSecond",
        r"\b1",
        r"\blu",
        r"\i1",
        r"\rOther",
        r"\p1",
        r"\q2",
        r"\fe128",
    ] {
        assert!(
            AssCodec
                .analyze(&script(&format!(r"{{\t(0,1000,{tags})}}X")))
                .is_err(),
            "{tags}"
        );
        assert!(
            AssCodec
                .analyze(&script(&format!(r"{{\t(\t({tags}))}}X")))
                .is_err(),
            "nested {tags}"
        );
    }
    assert!(AssCodec.analyze(&script(r"{\fe128}X")).is_err());
}
