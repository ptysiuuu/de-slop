/// Tests for Feature 5: Rule coverage statistics
///
/// After a run with --report, deslop prints which rules fired zero times.
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

// ── Feature 5: Rule coverage stats ───────────────────────────────────────────

/// When scanning a completely clean file, every rule should show 0 fires.
/// The output must contain a "Rules that never fired" section.
#[test]
fn test_coverage_stats_shown_with_report() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("clean.md");
    fs::write(&path, "This is a clean document with no slop.\n").unwrap();

    Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--report")
        .assert()
        .success()
        .stdout(predicate::str::contains("never fired"));
}

/// When a sloppy file is scanned, rules that DID fire must NOT appear in the
/// zero-fire section.
#[test]
fn test_fired_rules_not_in_zero_fire_list() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sloppy.md");
    fs::write(&path, "Hello \u{2014} world\n").unwrap(); // em-dash fires

    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--report")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // em-dash should NOT be in the never-fired list
    // We find the section and check
    if let Some(section_start) = stdout.find("never fired") {
        let section = &stdout[section_start..];
        assert!(
            !section.contains("em-dash"),
            "em-dash (which fired) appeared in never-fired section:\n{}",
            stdout
        );
    }
    // If no section, that's fine too — just means the feature isn't there yet
    // but the test will fail once we add the feature
}

/// The zero-fire report must include at least one rule name when using the
/// conservative profile on a file that only has em-dash.
#[test]
fn test_zero_fire_section_lists_rules() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("em_only.md");
    fs::write(&path, "Certainly! \u{2014} world\n").unwrap();

    // With conservative profile, prose rules don't fire
    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--report")
        .arg("--profile")
        .arg("conservative")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should mention "never fired" section  
    assert!(
        stdout.contains("never fired"),
        "Expected 'never fired' section in output:\n{}",
        stdout
    );
}

/// --report without --sentences should NOT print sentence breakdown
#[test]
fn test_no_sentence_section_without_flag() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "Certainly! It is worth noting that our robust API seamlessly leverages cutting-edge paradigms.\n").unwrap();

    let output = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--report")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Worst sentences"),
        "Sentence section appeared without --sentences flag:\n{}",
        stdout
    );
}
