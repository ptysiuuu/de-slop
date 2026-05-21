//! Watch mode — re-scans files on filesystem change events.
//!
//! Uses the `notify` crate with a 500ms debounce to avoid redundant rescans
//! when editors perform atomic saves (write-temp → rename).

use crate::detector::detect_file_type;
use crate::diff;
use crate::engine::process_file;
use crate::report::print_human_report;
use crate::rules::Rule;
use crate::scanner::is_binary;
use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Run the watch loop. Blocks until the user presses Ctrl-C.
pub fn run_watch(
    path: &Path,
    rules: &[Box<dyn Rule>],
    min_confidence: f32,
    recursive: bool,
) -> Result<()> {
    println!(
        "{} Watching {} (Ctrl-C to stop)",
        "◉".green(),
        path.display().to_string().bold()
    );

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    debouncer.watcher().watch(path, mode)?;

    // Track last-seen content per file so we can diff
    let mut last_content: HashMap<PathBuf, String> = HashMap::new();

    // Initial scan
    if path.is_file() {
        scan_and_print(path, rules, min_confidence, &mut last_content);
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                for event in events {
                    if event.kind == DebouncedEventKind::Any {
                        let file_path = &event.path;
                        if file_path.is_file() {
                            scan_and_print(file_path, rules, min_confidence, &mut last_content);
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("{} Watch error: {}", "✗".red(), e);
            }
            Err(e) => {
                eprintln!("{} Channel error: {}", "✗".red(), e);
                break;
            }
        }
    }

    Ok(())
}

fn scan_and_print(
    path: &Path,
    rules: &[Box<dyn Rule>],
    min_confidence: f32,
    last_content: &mut HashMap<PathBuf, String>,
) {
    let content_bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} Cannot read {}: {}", "✗".red(), path.display(), e);
            return;
        }
    };

    if is_binary(&content_bytes) {
        return;
    }

    let content = String::from_utf8_lossy(&content_bytes).to_string();
    let ext = path.extension().and_then(|s| s.to_str());
    let file_type = detect_file_type(&path.to_string_lossy(), ext, None);

    let (modified, matches) = process_file(&content, rules, file_type, min_confidence, ext);

    let path_str = path.to_string_lossy().to_string();

    println!();
    println!("{} {}", "◉ Changed:".cyan().bold(), path_str.bold());

    if matches.is_empty() {
        println!("  {} clean", "✓".green());
    } else {
        print_human_report(&path_str, &matches, false);

        // Show diff if content changed
        let prev = last_content.get(path).cloned().unwrap_or_default();
        if !prev.is_empty() && prev != content {
            let d = diff::generate_diff(&prev, &modified, &path_str);
            if !d.trim().is_empty() {
                println!("{}", d);
            }
        }
    }

    last_content.insert(path.to_path_buf(), content);
}
