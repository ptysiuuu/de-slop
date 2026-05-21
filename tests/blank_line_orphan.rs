/// Tests for blank-line orphan problem (Feature 1)
///
/// Invariant from spec: "DeleteLine and DeleteBlock collapse consecutive blank
/// lines to at most one after removal."
use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

// ── helpers ──────────────────────────────────────────────────────────────────

fn deslop(content: &str, ext: &str, extra_args: &[&str]) -> String {
    let dir = tempdir().unwrap();
    let path = dir.path().join(format!("test.{}", ext));
    fs::write(&path, content).unwrap();

    let mut cmd = Command::cargo_bin("deslop").unwrap();
    cmd.arg(&path);
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.assert().success();
    fs::read_to_string(&path).unwrap()
}

fn deslop_code(content: &str, extra_args: &[&str]) -> String {
    deslop(content, "py", extra_args)
}

// ── Feature 1 tests ──────────────────────────────────────────────────────────

/// Deleting a trivial comment between two blank-separated sections must NOT
/// produce two consecutive blank lines.
#[test]
fn test_delete_between_blank_lines_no_double_blank() {
    let input = "x = 1\n\n# Initialize\ny = 2\n\nz = 3\n";
    // After deleting `# Initialize` the result must not contain \n\n\n
    let output = deslop_code(input, &["--profile", "code"]);
    assert!(
        !output.contains("\n\n\n"),
        "Got double blank line in:\n{:?}",
        output
    );
    // Content is still there
    assert!(output.contains("x = 1"));
    assert!(output.contains("y = 2"), "Missing 'y = 2'. Output was: {:?}", output);
    assert!(output.contains("z = 3"));
}

/// Deleting multiple adjacent trivial comments must produce at most one blank
/// line in the gap.
#[test]
fn test_delete_multiple_adjacent_comments_collapses_blanks() {
    let input = concat!(
        "a = 1\n",
        "\n",
        "# Initialize\n",
        "# Set\n",
        "b = 2\n",
        "\n",
        "c = 3\n",
    );
    let output = deslop_code(input, &["--profile", "code"]);
    assert!(
        !output.contains("\n\n\n"),
        "Got triple newline in:\n{:?}",
        output
    );
}

/// Deleting a trivial comment that is the last line of the file must not leave
/// a trailing blank line artifact (more than one trailing newline).
#[test]
fn test_delete_at_end_of_file_no_trailing_blank() {
    // comment is last line; only code above it
    let input = "x = 1\n# Initialize\n";
    let output = deslop_code(input, &["--profile", "code"]);
    // Must not end with more than one newline
    let trailing_newlines = output.chars().rev().take_while(|&c| c == '\n').count();
    assert!(
        trailing_newlines <= 1,
        "Trailing newlines = {}, output = {:?}",
        trailing_newlines,
        output
    );
}

/// Deleting a trivial comment that is the first line must not leave a leading
/// blank line.
#[test]
fn test_delete_at_start_of_file_no_leading_blank() {
    let input = "# Initialize\nx = 1\n";
    let output = deslop_code(input, &["--profile", "code"]);
    assert!(
        !output.starts_with('\n'),
        "Leading newline present: {:?}",
        output
    );
}

/// Exactly one blank line between sections is permissible and must be
/// preserved — only *extra* blanks collapse.
#[test]
fn test_single_blank_line_preserved() {
    // The comment is sandwiched with a single blank line on each side.
    // After deletion there should be exactly one blank line between the code blocks.
    let input = "x = 1\n\n# Initialize\n\ny = 2\n";
    let output = deslop_code(input, &["--profile", "code"]);
    // Should have exactly two newlines between the last char of line 1 and y
    // i.e. the output is "x = 1\n\ny = 2\n"
    assert_eq!(
        output, "x = 1\n\ny = 2\n",
        "Unexpected output: {:?}",
        output
    );
}

/// DeleteBlock (docstring filler) must also not orphan extra blank lines.
#[test]
fn test_delete_block_no_orphan_blanks() {
    let input = concat!(
        "def foo():\n",
        "    \"\"\"\n",
        "    This function processes data\n",
        "    \"\"\"\n",
        "\n",
        "    return 42\n",
    );
    let output = deslop_code(input, &["--profile", "code"]);
    assert!(
        !output.contains("\n\n\n"),
        "Triple newline in:\n{:?}",
        output
    );
}

/// Regression: a file with NO blank lines around the deleted comment must
/// not gain blank lines.
#[test]
fn test_no_blank_lines_added() {
    let input = "x = 1\n# Set x\ny = 2\n";
    let output = deslop_code(input, &["--profile", "code"]);
    // No blank lines should appear (they weren't there before)
    assert!(
        !output.contains("\n\n"),
        "Unexpected blank line in:\n{:?}",
        output
    );
}
