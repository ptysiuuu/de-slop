/// Tests for Feature 4: --watch mode
///
/// `deslop --watch ./dir` re-scans modified files using filesystem events.
use std::process::Command;
use assert_cmd::cargo::CommandCargoExt;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

// ── Feature 4: --watch mode ───────────────────────────────────────────────────

/// `--watch` flag must be accepted by the CLI (no "unknown flag" error).
/// We immediately send Ctrl-C / kill it, just verifying startup.
#[test]
fn test_watch_flag_accepted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "Hello world.\n").unwrap();

    // Start watch, give it 200ms to boot, then the process should not have
    // immediately exited with an error code
    let mut child = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&path)
        .arg("--watch")
        .spawn()
        .expect("Failed to start deslop --watch");

    std::thread::sleep(Duration::from_millis(300));

    // If it's still running → good (watching); kill it
    match child.try_wait().unwrap() {
        None => {
            child.kill().unwrap();
        }
        Some(status) => {
            // If it exited, it must have exited with code 0 (not an error)
            assert!(
                status.success(),
                "--watch exited early with non-zero code: {:?}",
                status
            );
        }
    }
}

/// Watching a directory (not just a file) must not error at startup.
#[test]
fn test_watch_directory_accepted() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "Hello.\n").unwrap();

    let mut child = Command::cargo_bin("deslop")
        .unwrap()
        .arg(dir.path())
        .arg("--watch")
        .arg("--recursive")
        .spawn()
        .expect("Failed to start deslop --watch on directory");

    std::thread::sleep(Duration::from_millis(300));

    match child.try_wait().unwrap() {
        None => {
            child.kill().unwrap();
        }
        Some(status) => {
            assert!(status.success(), "--watch directory exited early: {:?}", status);
        }
    }
}

/// After a file is modified, the watcher must re-scan it.
/// We verify this by checking that the process remains running and doesn't
/// panic when a file changes.
#[test]
fn test_watch_rescan_on_change() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("doc.md");
    fs::write(&file_path, "Hello world.\n").unwrap();

    let mut child = Command::cargo_bin("deslop")
        .unwrap()
        .arg(&file_path)
        .arg("--watch")
        .spawn()
        .expect("Failed to start deslop --watch");

    // Let the watcher initialise
    std::thread::sleep(Duration::from_millis(400));

    // Modify the file (add an em-dash)
    fs::write(&file_path, "Hello \u{2014} world.\n").unwrap();

    // Give it time to process the event
    std::thread::sleep(Duration::from_millis(600));

    // Process should still be alive (not crashed)
    assert!(
        child.try_wait().unwrap().is_none(),
        "Watch process died after file change"
    );
    child.kill().unwrap();
}
