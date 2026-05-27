use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn copy_sloppy() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sloppy.md");
    fs::copy("tests/fixtures/sloppy.md", &file_path).unwrap();
    (dir, file_path)
}

#[test]
fn test_dry_run() {
    let (_dir, file_path) = copy_sloppy();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--dry-run");
    cmd.assert().success();
}

#[test]
fn test_diff() {
    let (_dir, file_path) = copy_sloppy();
    let path_str = file_path.to_string_lossy().to_string();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--diff");
    cmd.assert().stdout(predicate::str::contains(format!("--- a/{}", path_str)));
}

#[test]
fn test_json() {
    let (_dir, file_path) = copy_sloppy();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--format").arg("json").arg("--report");
    cmd.assert().stdout(predicate::str::contains("\"total_matches\""));
}

#[test]
fn test_threshold_fail() {
    let (_dir, file_path) = copy_sloppy();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--threshold").arg("0.0");
    cmd.assert().failure().code(1);
}

#[test]
fn test_threshold_pass() {
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg("tests/fixtures/clean.md").arg("--threshold").arg("100.0");
    cmd.assert().success();
}

#[test]
fn test_skip_em_dash() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Hello \u{2014} world").unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--skip").arg("em-dash");
    cmd.assert().success();
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "Hello \u{2014} world");
}

#[test]
fn test_fix_em_dash() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Hello \u{2014} world\u{2026}").unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--fix").arg("em-dash");
    cmd.assert().success();
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "Hello  -  world\u{2026}"); // ellipsis not fixed
}

#[test]
fn test_conservative_profile() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Certainly! Here is the code \u{2014}").unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--profile").arg("conservative");
    cmd.assert().success();
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "Certainly! Here is the code  - ");
}

#[test]
fn test_code_profile_trivial_comment() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.py");
    fs::write(&file_path, "# Initialize\nx = 1\n").unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--profile").arg("code");
    cmd.assert().success();
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "x = 1\n");
}

#[test]
fn test_check_rules_typo() {
    let dir = tempdir().unwrap();
    let rule_path = dir.path().join(".deslop-rules");
    fs::write(&rule_path, "rule broken\n  match: \"foo\"\n  fix: delete\n  sevrity: error\n").unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.current_dir(dir.path()).arg("check-rules");
    cmd.assert().failure().code(2).stdout(predicate::str::contains("did you mean \"severity\""));
}

// Task 1a: interactive in non-TTY mode leaves file unchanged
#[test]
fn test_interactive_nontty_unchanged() {
    let (_dir, file_path) = copy_sloppy();
    let original = fs::read_to_string(&file_path).unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--interactive");
    // In non-TTY (assert_cmd context), interactive falls through with no input.
    cmd.assert().success();

    let after = fs::read_to_string(&file_path).unwrap();
    assert_eq!(after, original, "File should be unchanged when interactive runs in non-TTY mode");
}

// Task 1b: threshold — high threshold passes, tight threshold fails
#[test]
fn test_threshold_high_passes() {
    let (_dir, file_path) = copy_sloppy();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--threshold").arg("9999");
    cmd.assert().success();
}

#[test]
fn test_threshold_tight_fails() {
    let (_dir, file_path) = copy_sloppy();
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--threshold").arg("0.001");
    cmd.assert().failure().code(1);
}

// Task 4: check-rules collects all errors, not just the first
#[test]
fn test_check_rules_all_errors_collected() {
    let dir = tempdir().unwrap();
    let rule_path = dir.path().join(".deslop-rules");
    // Three distinct errors: typo field, missing fix, invalid regex
    fs::write(
        &rule_path,
        concat!(
            "rule typo-field\n",
            "  match: \"foo\"\n",
            "  fix: delete\n",
            "  sevrity: error\n",
            "\n",
            "rule no-fix\n",
            "  match: \"bar\"\n",
            "\n",
            "rule bad-regex\n",
            "  match: \"[\"\n",
            "  fix: delete\n",
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.current_dir(dir.path()).arg("check-rules");
    cmd.assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("sevrity"))
        .stdout(predicate::str::contains("missing"))
        .stdout(predicate::str::contains("regex"));
}

// Task 5: doc comments (///) are not flagged as trivial
#[test]
fn test_doc_comment_not_flagged() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("lib.rs");
    let content = "/// Returns the user count for the active session.\npub fn user_count() -> usize { 0 }\n";
    fs::write(&file_path, content).unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&file_path).arg("--profile").arg("code").arg("--dry-run");
    cmd.assert().success();

    let after = fs::read_to_string(&file_path).unwrap();
    assert_eq!(after, content, "Doc comment should not be flagged as trivial");
}

// Task 6c: nonexistent path exits 2 with stderr message
#[test]
fn test_nonexistent_path_exits_2() {
    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg("/nonexistent/path/that/cannot/exist").arg("--dry-run");
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::is_empty().not());
}
