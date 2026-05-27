use crate::detector::{extract_comment_spans, FileType};
use crate::rules::{Category, FileTypeFilter, Match, MatchKind, Rule};
use std::cmp::Reverse;

pub fn process_file(
    content: &str,
    rules: &[Box<dyn Rule>],
    file_type: FileType,
    min_confidence: f32,
    file_ext: Option<&str>,
) -> (String, Vec<Match>) {
    let comment_spans = extract_comment_spans(content, file_ext);
    let mut all_matches = Vec::new();

    for (registry_index, rule) in rules.iter().enumerate() {
        let ftf = rule.file_type_filter();
        if ftf == FileTypeFilter::ProseOnly && file_type == FileType::Code {
            // allowed, but we'll filter to comments below
        } else if ftf == FileTypeFilter::CodeOnly && file_type != FileType::Code {
            continue;
        }

        let rule_matches = rule.check(content, Some(&comment_spans));
        for mut m in rule_matches {
            if m.confidence < min_confidence {
                continue;
            }

            if file_type == FileType::Code && m.category == Category::Prose {
                let inside_comment = comment_spans
                    .iter()
                    .any(|s| s.contains_range(&m.byte_range));
                if !inside_comment {
                    continue;
                }
            }

            if matches!(m.kind, MatchKind::DeleteLine | MatchKind::DeleteBlock) {
                let start = content[..m.byte_range.start]
                    .rfind('\n')
                    .map(|idx| idx + 1)
                    .unwrap_or(0);
                    
                let mut search_end = m.byte_range.end;
                if search_end > 0 && search_end <= content.len() && content.as_bytes()[search_end - 1] == b'\n' {
                    search_end -= 1;
                }
                
                let end = content[search_end..]
                    .find('\n')
                    .map(|idx| search_end + idx + 1)
                    .unwrap_or(content.len());
                m.byte_range = start..end;
            }

            all_matches.push((registry_index, m));
        }
    }

    all_matches.sort_by_key(|(_, m)| m.byte_range.start);

    let mut resolved = Vec::new();
    let mut i = 0;
    while i < all_matches.len() {
        let mut group = vec![all_matches[i].clone()];
        let mut max_end = all_matches[i].1.byte_range.end;
        let mut j = i + 1;
        while j < all_matches.len() && all_matches[j].1.byte_range.start < max_end {
            group.push(all_matches[j].clone());
            max_end = max_end.max(all_matches[j].1.byte_range.end);
            j += 1;
        }

        group.sort_by_key(|(idx, _)| *idx);
        resolved.push(group.remove(0).1);
        i = j;
    }

    let result_content = apply_matches(content, &resolved);

    resolved.sort_by_key(|m| m.byte_range.start);
    (result_content, resolved)
}

pub fn apply_matches(content: &str, matches: &[Match]) -> String {
    let mut result_content = content.to_string();
    let mut needs_collapse = false;
    
    // Sort reverse byte order for application
    let mut sorted_matches = matches.to_vec();
    sorted_matches.sort_by_key(|m| Reverse(m.byte_range.start));

    for m in &sorted_matches {
        if let MatchKind::FlagOnly = m.kind {
            continue;
        }
        if matches!(m.kind, MatchKind::DeleteLine | MatchKind::DeleteBlock) {
            needs_collapse = true;
        }
        apply_fix(&mut result_content, m);
    }
    
    if needs_collapse {
        collapse_blank_lines(&mut result_content);
    }

    result_content
}

fn apply_fix(content: &mut String, m: &Match) {
    match &m.kind {
        MatchKind::Replace(s) => {
            content.replace_range(m.byte_range.clone(), s);
        }
        MatchKind::DeleteLine | MatchKind::DeleteBlock => {
            content.replace_range(m.byte_range.clone(), "");
        }
        MatchKind::FlagOnly => {}
    }
}

/// Collapse consecutive blank lines (3+ `\n` in a row) to at most 2 (`\n\n`).
///
/// This satisfies the spec invariant:
/// > "DeleteLine and DeleteBlock collapse consecutive blank lines to at most one."
///
/// Two consecutive `\n` characters produce exactly one visible blank line, so
/// `\n\n` is the maximum we allow. Any run of 3+ newlines is replaced with 2.
fn collapse_blank_lines(content: &mut String) {
    // Fast path: if there is no triple-newline, nothing to do.
    if !content.contains("\n\n\n") {
        return;
    }

    // Replace all runs of 3+ newlines (possibly mixed with spaces/tabs on
    // blank lines) with exactly "\n\n".
    // We do this with a simple state-machine scan to avoid a regex dependency
    // in a hot path.
    let mut result = String::with_capacity(content.len());
    let mut newline_run = 0usize;
    let mut i = 0;
    let bytes = content.as_bytes();

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            newline_run += 1;
            // Consume any blank line content (spaces/tabs) between newlines
            // so that "  \n  \n  \n" also collapses correctly.
            i += 1;
            // Skip trailing whitespace on the current blank line only if it
            // leads directly to another newline (i.e. this is a truly blank line).
            if i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\r') {
                let mut j = i;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\r') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'\n' {
                    i = j;
                }
            }
        } else {
            // Flush accumulated newlines — cap at 2
            let emit = newline_run.min(2);
            for _ in 0..emit {
                result.push('\n');
            }
            newline_run = 0;
            result.push(b as char);
            i += 1;
        }
    }
    // Flush trailing newlines — cap at 1 (no trailing blank lines)
    let emit = newline_run.min(1);
    for _ in 0..emit {
        result.push('\n');
    }

    *content = result;
}
