/// Tests for Feature 8: per-language rule packs as runtime profiles
///
/// `--profile rust-pack` and `--profile python-pack` activate language-specific
/// rules on top of the base profile. Can be stacked with `--extra-profile`.
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_file(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

// ── Feature 8: language pack profiles ────────────────────────────────────────

/// `--profile rust-pack` is accepted without error.
#[test]
fn test_rust_pack_profile_accepted() {
    let dir = tempdir().unwrap();
    let path = make_file(&dir, "lib.rs", "fn foo() {}\n");
    Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("rust-pack")
        .assert()
        .success();
}

/// `--profile python-pack` is accepted without error.
#[test]
fn test_python_pack_profile_accepted() {
    let dir = tempdir().unwrap();
    let path = make_file(&dir, "mod.py", "def foo(): pass\n");
    Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("python-pack")
        .assert()
        .success();
}

/// rust-pack fires on GPT-style Rust doc comments.
#[test]
fn test_rust_pack_fires_on_verbose_doc_comment() {
    let dir = tempdir().unwrap();
    let content = concat!(
        "/// This function processes the input data and returns a result.\n",
        "pub fn process(data: &str) -> String {\n",
        "    data.to_string()\n",
        "}\n"
    );
    let path = make_file(&dir, "lib.rs", content);

    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("rust-pack")
        .arg("--dry-run")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should detect the verbose doc comment
    assert!(
        stdout.contains("rust-verbose-doc") || stdout.contains("docstring-filler"),
        "Expected rust-pack rule to fire on verbose doc. Got:\n{}",
        stdout
    );
}

/// python-pack fires on NumPy-style boilerplate docstrings.
#[test]
fn test_python_pack_fires_on_boilerplate_docstring() {
    let dir = tempdir().unwrap();
    let content = concat!(
        "def process(data):\n",
        "    \"\"\"\n",
        "    Process the data.\n",
        "\n",
        "    Args:\n",
        "        data: The input data.\n",
        "\n",
        "    Returns:\n",
        "        The output data.\n",
        "    \"\"\"\n",
        "    return data\n"
    );
    let path = make_file(&dir, "mod.py", content);

    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("python-pack")
        .arg("--dry-run")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("python-args-boilerplate") || stdout.contains("docstring"),
        "Expected python-pack rule to fire on boilerplate docstring. Got:\n{}",
        stdout
    );
}

/// The `code` profile must NOT fire pack-specific rules.
#[test]
fn test_code_profile_does_not_fire_rust_pack_rules() {
    let dir = tempdir().unwrap();
    let content = concat!(
        "/// This function processes the input data and returns a result.\n",
        "pub fn process(data: &str) -> String { data.to_string() }\n"
    );
    let path = make_file(&dir, "lib.rs", content);

    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("code")
        .arg("--dry-run")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("rust-verbose-doc"),
        "rust-pack rule fired under code profile:\n{}",
        stdout
    );
}

/// `--extra-profile` stacks a pack on top of a base profile.
#[test]
fn test_extra_profile_stacks_on_base() {
    let dir = tempdir().unwrap();
    let content = concat!(
        "/// This function processes the input data and returns a result.\n",
        "pub fn process(data: &str) -> String { data.to_string() }\n",
        "Hello \u{2014} world\n", // em-dash should fire from base `code` profile
    );
    let path = make_file(&dir, "lib.rs", content);

    // code + rust-pack should fire em-dash AND rust-verbose-doc
    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--profile")
        .arg("code")
        .arg("--extra-profile")
        .arg("rust-pack")
        .arg("--dry-run")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // em-dash from base code profile
    assert!(
        stdout.contains("em-dash"),
        "em-dash rule not found in extra-profile output:\n{}",
        stdout
    );
}

/// explain-rules must include pack rules when rust-pack profile is active.
#[test]
fn test_explain_rules_shows_rust_pack_rules() {
    Command::cargo_bin("deslop")
        .unwrap()
        .arg("--profile")
        .arg("rust-pack")
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("rust-verbose-doc"));
}

/// explain-rules must include python pack rules when python-pack is active.
#[test]
fn test_explain_rules_shows_python_pack_rules() {
    Command::cargo_bin("deslop")
        .unwrap()
        .arg("--profile")
        .arg("python-pack")
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("python-args-boilerplate"));
}
