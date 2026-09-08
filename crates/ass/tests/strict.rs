use assfonts_ass::{AssCodec, ConfiguredAssCodec, ParseMode};
use assfonts_core::SubtitleCodec;

fn script(text: &str) -> String {
    include_str!("../../../examples/basic.ass").replace("Hello", text)
}

#[test]
fn strict_is_default_and_rejects_typos_unknown_tags_and_nonstandard_integers() {
    assert_eq!(AssCodec.parse_mode(), ParseMode::Strict);
    assert_eq!(
        ConfiguredAssCodec::default().parse_mode(),
        ParseMode::Strict
    );
    for tag in [
        r"\blu0.8",
        r"\bklur0.5",
        r"\unknown",
        r"\border3",
        r"\N",
        r"\b1.7",
        r"\b+1tail",
        r"\b(1)",
        r"\b2147483648",
        r"\i2",
        r"\q9",
        r"\pOops",
        r"\p1.2",
        r"\i-1",
    ] {
        let input = script(&format!("{{{tag}}}Hello"));
        let error = AssCodec.analyze(&input).unwrap_err().to_string();
        assert!(error.contains("strict syntax"), "{tag}: {error}");
        assert!(error.contains("dialogue 1"), "{error}");
        assert!(AssCodec::with_mode(ParseMode::Strict)
            .analyze(&input)
            .is_err());
    }
    for tag in [
        r"\blu0.8",
        r"\bklur0.5",
        r"\unknown",
        r"\b1.7",
        r"\i2",
        r"\q9",
        r"\pOops",
    ] {
        AssCodec::with_mode(ParseMode::Compatible)
            .analyze(&script(&format!("{{{tag}}}Hello")))
            .unwrap();
    }
}

#[test]
fn strict_keeps_nested_visual_animations_and_common_recovery_forms() {
    for tags in [
        r"\t(0,1000,\frz5\t(500,1000,\frz0)\t(900,1000,\alpha&HFF&))",
        r"\t(\t(\fs40)\fnOpen Sans\b400",
        r"\t(0,1000,\t(\blur0.8)",
        r"\t(100,6590)",
        r"\t()",
        r"\\blur0.5",
        r"\fnOpen Sans\rDefault\b-1\b20\b400\i0\q3\p0",
        r"\1c000000\3cFFFFFF\alpha&HFF&",
    ] {
        let input = script(&format!("{{{tags}}}Hello"));
        assert_eq!(
            AssCodec.analyze(&input).unwrap(),
            AssCodec::with_mode(ParseMode::Compatible)
                .analyze(&input)
                .unwrap(),
            "{tags}"
        );
    }
}

#[test]
fn strict_checks_nested_tags_but_does_not_reinterpret_animation_boundaries() {
    for tags in [r"\unknown", r"\blu0.8", r"\bklur0.5"] {
        let input = script(&format!(r"{{\t(\t({tags}))}}Hello"));
        let error = AssCodec.analyze(&input).unwrap_err().to_string();
        assert!(error.contains("unknown tag"), "{error}");
    }
    // The b is outside the transform under the renderer's first-close rule.
    let input = script(r"{\t(\t(\fs40)\b1.7}Hello");
    assert!(AssCodec
        .analyze(&input)
        .unwrap_err()
        .to_string()
        .contains("exact i32"));
    AssCodec::with_mode(ParseMode::Compatible)
        .analyze(&input)
        .unwrap();
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let boundary = format!("{}\\fs20{}", "\\t(".repeat(65), ")".repeat(65));
        AssCodec::with_mode(mode)
            .analyze(&script(&format!("{{{boundary}}}Hello")))
            .unwrap();
        for tags in [
            r"\fnOther",
            r"\b1",
            r"\i1",
            r"\r",
            r"\p1",
            r"\q2",
            r"\fe128",
        ] {
            assert!(AssCodec::with_mode(mode)
                .analyze(&script(&format!(r"{{\t(\t({tags}))}}Hello")))
                .is_err());
        }
        let deep = format!("{}\\fs20{}", "\\t(".repeat(66), ")".repeat(66));
        assert!(AssCodec::with_mode(mode)
            .analyze(&script(&format!("{{{deep}}}Hello")))
            .is_err());
    }
}

#[test]
fn strict_diagnostics_keep_utf8_offsets_in_nested_input() {
    let text = r"中文{\t(0,100,\t(\bklur0.5))}Hello";
    let error = AssCodec.analyze(&script(text)).unwrap_err().to_string();
    let offset = text.find(r"\bklur").unwrap();
    assert!(
        error.contains(&format!("dialogue text byte {offset}:")),
        "{error}"
    );
    assert!(error.contains(r"\bklur0.5"), "{error}");
}
