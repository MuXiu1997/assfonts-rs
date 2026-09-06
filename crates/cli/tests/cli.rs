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
