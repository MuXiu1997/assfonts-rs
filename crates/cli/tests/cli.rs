#![cfg(feature = "harfbuzz")]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const ASS: &str = include_str!("../../../examples/basic.ass");
fn font() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf")
}
fn invoke(input: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_assfonts-rs"))
        .arg("-i")
        .arg(input)
        .arg("-f")
        .arg(font())
        .arg("-o")
        .arg(out)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn actual_cli_embeds_reports_and_refuses_overwrite() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.ass");
    let out = tmp.path().join("out");
    fs::write(&input, ASS).unwrap();
    let first = invoke(&input, &out, &["--json", "-v0"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert!(json["files"][0]["backend"]
        .as_str()
        .unwrap()
        .contains("14.4.0"));
    assert_eq!(json["files"][0]["fonts"].as_array().unwrap().len(), 1);
    let output = out.join("input.assfonts.ass");
    let before = fs::read(&output).unwrap();
    assert!(String::from_utf8_lossy(&before).contains("[Fonts]"));
    assert!(!invoke(&input, &out, &[]).status.success());
    assert_eq!(fs::read(&output).unwrap(), before);
    assert!(invoke(&input, &out, &["--overwrite"]).status.success());
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_eq!(fs::read_to_string(&input).unwrap(), ASS);
}

#[test]
fn check_mode_is_read_only_and_failure_propagates() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.ass");
    let out = tmp.path().join("does-not-exist");
    fs::write(&input, ASS).unwrap();
    assert!(invoke(&input, &out, &["--check"]).status.success());
    assert!(!out.exists());
    fs::write(&input, ASS.replace("Open Sans", "Missing Font")).unwrap();
    assert!(!invoke(&input, &out, &["--check"]).status.success());
    assert!(!out.exists());
    fs::write(&input, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    assert!(!invoke(&input, &out, &[]).status.success());
    assert!(!out.join("input.assfonts.ass").exists());
}

#[test]
fn batch_processing_failure_publishes_nothing() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("inputs");
    let out = tmp.path().join("out");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("a.ass"), ASS).unwrap();
    fs::write(
        input.join("b.ass"),
        ASS.replace("Open Sans", "Missing Font"),
    )
    .unwrap();
    assert!(!invoke(&input, &out, &[]).status.success());
    assert!(!out.join("a.assfonts.ass").exists());
}

#[test]
fn missing_glyph_policy_defaults_to_warn_for_check_and_embed() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.ass");
    let out = tmp.path().join("out");
    let text = ASS.replace("Hello", "\u{378}Hello");
    assert_ne!(text, ASS);
    fs::write(&input, &text).unwrap();
    assert!(
        !invoke(&input, &out, &["--check", "--missing-glyphs", "error"])
            .status
            .success()
    );
    assert!(!invoke(&input, &out, &["--missing-glyphs", "error"])
        .status
        .success());
    let check = invoke(&input, &out, &["--check", "--json"]);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(!out.join("input.assfonts.ass").exists());
    let checked: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(checked["files"][0]["warnings"][0]["characters"], "\u{378}");
    let explicit_check = invoke(
        &input,
        &out,
        &["--check", "--missing-glyphs", "warn", "--json"],
    );
    assert!(explicit_check.status.success());
    assert_eq!(check.stdout, explicit_check.stdout);
    let embedded = invoke(&input, &out, &["--json"]);
    assert!(embedded.status.success());
    let report: serde_json::Value = serde_json::from_slice(&embedded.stdout).unwrap();
    assert_eq!(report["files"][0]["missing_glyph_policy"], "warn");
    assert_eq!(
        report["files"][0]["warnings"],
        checked["files"][0]["warnings"]
    );
    assert!(String::from_utf8_lossy(&embedded.stderr).contains("warning"));
    let default_bytes = fs::read(out.join("input.assfonts.ass")).unwrap();
    assert!(
        invoke(&input, &out, &["--missing-glyphs", "warn", "--overwrite"])
            .status
            .success()
    );
    assert_eq!(
        default_bytes,
        fs::read(out.join("input.assfonts.ass")).unwrap()
    );
    let help = Command::new(env!("CARGO_BIN_EXE_assfonts-rs"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("[default: warn]"));
    assert_eq!(fs::read_to_string(&input).unwrap(), text);
    fs::write(&input, ASS.replace("Open Sans", "Missing Font")).unwrap();
    assert!(
        !invoke(&input, &out, &["--check", "--missing-glyphs", "warn"])
            .status
            .success()
    );
}

#[test]
fn report_cannot_replace_a_source_or_subtitle_output() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.ass");
    let out = tmp.path().join("out");
    fs::write(&input, ASS).unwrap();
    assert!(!invoke(
        &input,
        &out,
        &["--report", input.to_str().unwrap(), "--overwrite"]
    )
    .status
    .success());
    assert!(!invoke(
        &input,
        &out,
        &["--report", font().to_str().unwrap(), "--overwrite"]
    )
    .status
    .success());
    assert!(!invoke(
        &input,
        &out,
        &["--report", out.join("input.assfonts.ass").to_str().unwrap()]
    )
    .status
    .success());
    assert_eq!(fs::read_to_string(input).unwrap(), ASS);
}

#[test]
fn same_stem_batch_outputs_and_unavailable_backend_are_errors() {
    let tmp = TempDir::new().unwrap();
    for sub in ["a", "b"] {
        fs::create_dir(tmp.path().join(sub)).unwrap();
        fs::write(tmp.path().join(sub).join("input.ass"), ASS).unwrap();
    }
    let out = tmp.path().join("out");
    assert!(!invoke(tmp.path(), &out, &[]).status.success());
    assert!(!invoke(
        &tmp.path().join("a/input.ass"),
        &out,
        &["--backend", "missing"]
    )
    .status
    .success());
}

#[cfg(unix)]
#[test]
fn output_symlinks_are_not_followed_even_with_overwrite() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.ass");
    fs::write(&input, ASS).unwrap();
    std::os::unix::fs::symlink(&input, tmp.path().join("input.assfonts.ass")).unwrap();
    assert!(!invoke(&input, tmp.path(), &["--overwrite"])
        .status
        .success());
    assert_eq!(fs::read_to_string(input).unwrap(), ASS);
}
