/// Tests for Feature 3: `deslop lsp` subcommand
///
/// Starts a JSON-RPC Language Server Protocol daemon on stdin/stdout.
/// Tests use in-process JSON-RPC message exchange.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::assert::OutputAssertExt;
use predicates::prelude::*;

// ── LSP test helpers ──────────────────────────────────────────────────────────

fn lsp_message(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn read_lsp_response(reader: &mut impl BufRead) -> serde_json::Value {
    // Read headers
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end_matches("\r\n").trim_end_matches('\n');
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length: ") {
            content_length = rest.trim().parse().unwrap();
        }
    }
    // Read body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).expect("LSP response is not valid JSON")
}

fn spawn_lsp() -> std::process::Child {
    // Find the binary path via cargo
    let bin = assert_cmd::cargo::cargo_bin("deslop");
    Command::new(bin)
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn deslop lsp")
}

// ── Feature 3 tests ───────────────────────────────────────────────────────────

/// `deslop lsp` starts and responds to an `initialize` request with
/// `InitializeResult`.
#[test]
fn test_lsp_initialize() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": null,
            "capabilities": {}
        }
    });
    let body = init_request.to_string();
    stdin.write_all(lsp_message(&body).as_bytes()).unwrap();
    stdin.flush().unwrap();

    // Give the server time to respond
    std::thread::sleep(Duration::from_millis(500));

    let response = read_lsp_response(&mut reader);
    assert_eq!(response["id"], 1, "Response ID mismatch");
    assert!(
        response["result"]["capabilities"].is_object(),
        "Expected capabilities in InitializeResult: {:?}",
        response
    );

    child.kill().unwrap();
}

/// After `initialize` + `initialized`, sending `textDocument/didOpen` with
/// sloppy content must produce a `textDocument/publishDiagnostics` notification
/// with at least one diagnostic.
#[test]
fn test_lsp_did_open_sloppy_produces_diagnostics() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    });
    stdin.write_all(lsp_message(&init.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let _init_resp = read_lsp_response(&mut reader);

    // initialized notification
    let initialized = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialized", "params": {}
    });
    stdin.write_all(lsp_message(&initialized.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // didOpen with sloppy content
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/test.md",
                "languageId": "markdown",
                "version": 1,
                "text": "Certainly! It is worth noting that our robust API seamlessly leverages cutting-edge paradigms.\n"
            }
        }
    });
    stdin.write_all(lsp_message(&did_open.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    // Read publishDiagnostics notification
    let notification = read_lsp_response(&mut reader);
    assert_eq!(
        notification["method"], "textDocument/publishDiagnostics",
        "Expected publishDiagnostics, got: {:?}",
        notification
    );
    let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
    assert!(
        !diagnostics.is_empty(),
        "Expected at least one diagnostic for sloppy content"
    );

    child.kill().unwrap();
}

/// `textDocument/didOpen` with clean content must produce publishDiagnostics
/// with an empty diagnostics array.
#[test]
fn test_lsp_did_open_clean_produces_no_diagnostics() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    });
    stdin.write_all(lsp_message(&init.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let _init_resp = read_lsp_response(&mut reader);

    // initialized
    let initialized = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialized", "params": {}
    });
    stdin.write_all(lsp_message(&initialized.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // didOpen with clean content
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/clean.md",
                "languageId": "markdown",
                "version": 1,
                "text": "The quick brown fox jumps over the lazy dog.\n"
            }
        }
    });
    stdin.write_all(lsp_message(&did_open.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let notification = read_lsp_response(&mut reader);
    assert_eq!(notification["method"], "textDocument/publishDiagnostics");
    let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.is_empty(),
        "Expected empty diagnostics for clean content, got: {:?}",
        diagnostics
    );

    child.kill().unwrap();
}

/// FlagOnly matches must produce Warning-severity diagnostics (severity 2),
/// not Error (severity 1).
#[test]
fn test_lsp_flag_only_matches_are_warnings() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    });
    stdin.write_all(lsp_message(&init.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let _init_resp = read_lsp_response(&mut reader);

    let initialized = serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}});
    stdin.write_all(lsp_message(&initialized.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // "robust" and "seamlessly" are FlagOnly hollow intensifiers
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/flagonly.md",
                "languageId": "markdown",
                "version": 1,
                "text": "Our robust and seamlessly scalable system.\n"
            }
        }
    });
    stdin.write_all(lsp_message(&did_open.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let notification = read_lsp_response(&mut reader);
    let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();

    // All diagnostics for FlagOnly must have severity 2 (Warning)
    for diag in diagnostics {
        // LSP DiagnosticSeverity: 1=Error, 2=Warning, 3=Information, 4=Hint
        let severity = diag["severity"].as_u64().unwrap_or(1);
        assert_eq!(
            severity, 2,
            "FlagOnly match should be Warning (2), got {}: {:?}",
            severity, diag
        );
    }

    child.kill().unwrap();
}

/// High-confidence (>= 0.85) non-FlagOnly matches must be Error severity (1).
#[test]
fn test_lsp_high_confidence_matches_are_errors() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    });
    stdin.write_all(lsp_message(&init.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let _init_resp = read_lsp_response(&mut reader);

    let initialized = serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}});
    stdin.write_all(lsp_message(&initialized.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // em-dash has confidence 1.0 — should be Error
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/emdash.md",
                "languageId": "markdown",
                "version": 1,
                "text": "Hello \u{2014} world.\n"
            }
        }
    });
    stdin.write_all(lsp_message(&did_open.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let notification = read_lsp_response(&mut reader);
    let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "Expected em-dash diagnostic");

    // Find the em-dash diagnostic
    let em_dash_diag = diagnostics
        .iter()
        .find(|d| d["source"].as_str() == Some("em-dash"))
        .expect("em-dash diagnostic not found");
    assert_eq!(
        em_dash_diag["severity"].as_u64().unwrap(),
        1,
        "em-dash (confidence 1.0) should be Error (1)"
    );

    child.kill().unwrap();
}

/// `deslop lsp --help` must exit 0 and show LSP-specific help.
#[test]
fn test_lsp_subcommand_has_help() {
    Command::cargo_bin("deslop")
        .unwrap()
        .arg("lsp")
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("lsp")
                .or(predicate::str::contains("Language Server"))
        );
}

/// After `didOpen` with sloppy content, a `textDocument/codeAction` request
/// for the diagnostic range must return at least one CodeAction with a
/// non-null `edit` field.
#[test]
fn test_lsp_code_action_returns_edit() {
    let mut child = spawn_lsp();
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    });
    stdin.write_all(lsp_message(&init.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let _init_resp = read_lsp_response(&mut reader);

    // initialized
    let initialized = serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}});
    stdin.write_all(lsp_message(&initialized.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // didOpen with content containing an em-dash (Replace kind — produces a fix)
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/codeaction_test.md",
                "languageId": "markdown",
                "version": 1,
                "text": "Hello \u{2014} world.\n"
            }
        }
    });
    stdin.write_all(lsp_message(&did_open.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    // Read publishDiagnostics and grab the first diagnostic's range
    let notification = read_lsp_response(&mut reader);
    assert_eq!(notification["method"], "textDocument/publishDiagnostics");
    let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic for em-dash content");
    let diag_range = &diagnostics[0]["range"];

    // Send codeAction request for the diagnostic range
    let code_action_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": { "uri": "file:///tmp/codeaction_test.md" },
            "range": diag_range,
            "context": { "diagnostics": [diagnostics[0].clone()], "only": [] }
        }
    });
    stdin.write_all(lsp_message(&code_action_req.to_string()).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let response = read_lsp_response(&mut reader);
    assert_eq!(response["id"], 2, "Response ID should match request");
    let actions = response["result"].as_array().expect("Expected array of code actions");
    assert!(!actions.is_empty(), "Expected at least one code action for em-dash diagnostic");

    let first_action = &actions[0];
    assert!(
        !first_action["edit"].is_null(),
        "Code action must have a non-null edit: {:?}",
        first_action
    );

    child.kill().unwrap();
}
