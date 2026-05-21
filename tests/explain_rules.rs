/// Tests for Feature 7: `deslop explain-rules` subcommand
///
/// Prints every active rule with: ID, pattern, fix behaviour, confidence, example.
use assert_cmd::Command;
use predicates::prelude::*;

// ── Feature 7: explain-rules subcommand ─────────────────────────────────────

/// `deslop explain-rules` must exit 0.
#[test]
fn test_explain_rules_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("explain-rules")
        .assert()
        .success();
}

/// Output must contain every built-in rule ID.
#[test]
fn test_explain_rules_contains_all_builtin_ids() {
    let expected_ids = [
        "em-dash",
        "en-dash",
        "curly-quotes",
        "ellipsis",
        "nbsp",
        "zero-width",
        "soft-hyphen",
        "emoji",
        "decorative-unicode",
        "filler-openers",
        "sycophantic-closers",
        "hedges",
        "transition-padding",
        "hollow-intensifiers",
        "trivial-comment",
        "docstring-filler",
    ];
    let dir = tempfile::tempdir().unwrap();
    for id in &expected_ids {
        Command::cargo_bin("deslop")
            .unwrap()
            .current_dir(dir.path())
            .arg("--profile")
            .arg("aggressive")
            .arg("explain-rules")
            .assert()
            .success()
            .stdout(predicate::str::contains(*id));
    }
}

/// Output must contain confidence values (e.g. "1.00" or "0.95").
#[test]
fn test_explain_rules_contains_confidence() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("1.00").or(predicate::str::contains("0.95")));
}

/// Output must contain example input/output strings.
#[test]
fn test_explain_rules_contains_examples() {
    let dir = tempfile::tempdir().unwrap();
    // The em-dash rule example must show U+2014 mapped to " - "
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{2014}"));
}

/// Output must contain fix behaviour descriptions.
#[test]
fn test_explain_rules_contains_fix_behavior() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Replace")
                .or(predicate::str::contains("Delete"))
                .or(predicate::str::contains("FlagOnly")),
        );
}

/// With --profile conservative, prose rules must be absent from output.
#[test]
fn test_explain_rules_respects_profile() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("--profile")
        .arg("conservative")
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("filler-openers").not());
}

/// With --profile aggressive, all rules must appear.
#[test]
fn test_explain_rules_aggressive_shows_code_rules() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("deslop")
        .unwrap()
        .current_dir(dir.path())
        .arg("--profile")
        .arg("aggressive")
        .arg("explain-rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("trivial-comment"))
        .stdout(predicate::str::contains("filler-openers"));
}
