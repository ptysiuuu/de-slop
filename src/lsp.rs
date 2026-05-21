//! LSP server mode — `deslop lsp`
//!
//! Implements a Language Server Protocol server that speaks JSON-RPC over
//! stdio. Editors call `deslop lsp` and connect via the standard LSP client
//! protocol.
//!
//! Supported LSP methods:
//! - `initialize` / `initialized` / `shutdown` / `exit`
//! - `textDocument/didOpen` → scan → `textDocument/publishDiagnostics`
//! - `textDocument/didChange` → scan → `textDocument/publishDiagnostics`
//! - `textDocument/didClose` → clear diagnostics

use crate::detector::detect_file_type;
use crate::engine::process_file;
use crate::rules::{Match, MatchKind, Rule};
use anyhow::Result;
use lsp_server::{Connection, Message, Response};
use lsp_types::*;
use std::collections::HashMap;

// LSP notification method strings
const DID_OPEN: &str = "textDocument/didOpen";
const DID_CHANGE: &str = "textDocument/didChange";
const DID_CLOSE: &str = "textDocument/didClose";
const PUBLISH_DIAGNOSTICS: &str = "textDocument/publishDiagnostics";

/// Run the LSP server. Blocks until the client sends `shutdown` + `exit`.
pub fn run_lsp_server(rules: Vec<Box<dyn Rule>>, min_confidence: f32) -> Result<()> {
    // Create a connection on stdin/stdout (standard LSP transport)
    let (connection, io_threads) = Connection::stdio();

    // Perform the LSP handshake (initialize request/response)
    let server_capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        ..Default::default()
    };

    let init_params = connection.initialize(serde_json::to_value(server_capabilities)?)?;
    let _init_params: InitializeParams = serde_json::from_value(init_params)?;

    // Track open documents
    let mut documents: HashMap<Uri, String> = HashMap::new();

    // Main message loop
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    break;
                }
                // We don't handle other requests for now
                let resp = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::MethodNotFound as i32,
                    format!("Method not found: {}", req.method),
                );
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Notification(notif) => {
                handle_notification(
                    &connection,
                    notif,
                    &mut documents,
                    &rules,
                    min_confidence,
                )?;
            }
            Message::Response(_) => {
                // We don't send requests to the client, so ignore responses
            }
        }
    }

    io_threads.join()?;
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    notif: lsp_server::Notification,
    documents: &mut HashMap<Uri, String>,
    rules: &[Box<dyn Rule>],
    min_confidence: f32,
) -> Result<()> {
    if notif.method == DID_OPEN {
        let params: DidOpenTextDocumentParams = serde_json::from_value(notif.params)?;
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        let language_id = params.text_document.language_id.clone();
        documents.insert(uri.clone(), text.clone());
        publish_diagnostics(connection, &uri, &text, &language_id, rules, min_confidence)?;
    } else if notif.method == DID_CHANGE {
        let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
        let uri = params.text_document.uri.clone();
        if let Some(last_change) = params.content_changes.into_iter().last() {
            let text = last_change.text;
            // Derive language_id from file extension in the URI path
            let uri_str = uri.to_string();
            let language_id = uri_str
                .rsplit('.')
                .next()
                .unwrap_or("txt")
                .to_string();
            documents.insert(uri.clone(), text.clone());
            publish_diagnostics(connection, &uri, &text, &language_id, rules, min_confidence)?;
        }
    } else if notif.method == DID_CLOSE {
        let params: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
        let uri = params.text_document.uri.clone();
        documents.remove(&uri);
        // Clear diagnostics for closed file
        let clear = PublishDiagnosticsParams {
            uri,
            diagnostics: vec![],
            version: None,
        };
        connection.sender.send(Message::Notification(
            lsp_server::Notification::new(
                PUBLISH_DIAGNOSTICS.to_string(),
                serde_json::to_value(clear)?,
            ),
        ))?;
    }
    // Ignore `initialized` and other notifications
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: &Uri,
    text: &str,
    language_id: &str,
    rules: &[Box<dyn Rule>],
    min_confidence: f32,
) -> Result<()> {
    // Detect file type from URI string
    let uri_str = uri.to_string();
    // The URI path is the part after the scheme+authority
    let fs_path: &str = uri_str
        .strip_prefix("file://")
        .unwrap_or(&uri_str);
    let ext = fs_path
        .rsplit('.')
        .next()
        .filter(|s| !s.contains('/'))
        .and_then(|s| if s.is_empty() { None } else { Some(s) });
    let file_type = detect_file_type(fs_path, ext, Some(language_id));

    let (_modified, matches) = process_file(text, rules, file_type, min_confidence, ext);

    let diagnostics: Vec<Diagnostic> = matches
        .iter()
        .map(|m| match_to_diagnostic(text, m))
        .collect();

    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics,
        version: None,
    };

    connection.sender.send(Message::Notification(
        lsp_server::Notification::new(
            PUBLISH_DIAGNOSTICS.to_string(),
            serde_json::to_value(params)?,
        ),
    ))?;

    Ok(())
}

fn match_to_diagnostic(content: &str, m: &Match) -> Diagnostic {
    // Convert byte range to LSP Position (line/character, 0-indexed)
    let (start_line, start_char) = byte_offset_to_lsp_position(content, m.byte_range.start);
    let (end_line, end_char) = byte_offset_to_lsp_position(content, m.byte_range.end.min(content.len()));

    // Severity:
    // - FlagOnly → Warning (2)
    // - confidence >= 0.85 and not FlagOnly → Error (1)  
    // - otherwise → Warning (2)
    let severity = if matches!(m.kind, MatchKind::FlagOnly) {
        DiagnosticSeverity::WARNING
    } else if m.confidence >= 0.85 {
        DiagnosticSeverity::ERROR
    } else {
        DiagnosticSeverity::WARNING
    };

    Diagnostic {
        range: Range {
            start: Position {
                line: start_line as u32,
                character: start_char as u32,
            },
            end: Position {
                line: end_line as u32,
                character: end_char as u32,
            },
        },
        severity: Some(severity),
        code: Some(NumberOrString::String(m.rule_id.clone())),
        source: Some(m.rule_id.clone()),
        message: m.description.clone(),
        ..Default::default()
    }
}

fn byte_offset_to_lsp_position(content: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    let safe_offset = byte_offset.min(content.len());
    for (i, ch) in content.char_indices() {
        if i >= safe_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16();
        }
    }
    (line, col)
}
