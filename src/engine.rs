use crate::detector::{extract_comment_spans, FileType};
use crate::rules::{Category, FileTypeFilter, Match, MatchKind, Rule};

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

            // Expand byte_range for DeleteLine and DeleteBlock to the actual line boundaries
            // so that overlap resolution and back-to-front processing work correctly.
            if matches!(m.kind, MatchKind::DeleteLine | MatchKind::DeleteBlock) {
                let start = content[..m.byte_range.start]
                    .rfind('\n')
                    .map(|idx| idx + 1)
                    .unwrap_or(0);
                let end = content[m.byte_range.end..]
                    .find('\n')
                    .map(|idx| m.byte_range.end + idx + 1)
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

    resolved.sort_by(|a, b| b.byte_range.start.cmp(&a.byte_range.start));

    let mut result_content = content.to_string();
    for m in &resolved {
        if let MatchKind::FlagOnly = m.kind {
            continue;
        }
        apply_fix(&mut result_content, m);
    }

    resolved.sort_by_key(|m| m.byte_range.start);
    (result_content, resolved)
}

fn apply_fix(content: &mut String, m: &Match) {
    match &m.kind {
        MatchKind::Replace(s) => {
            content.replace_range(m.byte_range.clone(), s);
        }
        MatchKind::DeleteLine | MatchKind::DeleteBlock => {
            content.replace_range(m.byte_range.clone(), "");
            collapse_blank_lines(content, m.byte_range.start);
        }
        MatchKind::FlagOnly => {}
    }
}

fn collapse_blank_lines(content: &mut String, at_offset: usize) {
    // Expand to find the full span of whitespace containing newlines
    let bytes = content.as_bytes();
    
    let mut span_start = at_offset;
    while span_start > 0 {
        let b = bytes[span_start - 1];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            span_start -= 1;
        } else {
            break;
        }
    }

    let mut span_end = at_offset;
    while span_end < bytes.len() {
        let b = bytes[span_end];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            span_end += 1;
        } else {
            break;
        }
    }

    let span_str = &content[span_start..span_end];
    let newlines = span_str.chars().filter(|&c| c == '\n').count();
    
    // If there are > 2 newlines (which means >= 2 consecutive blank lines)
    if newlines > 2 {
        // Collapse to exactly \n\n (which visually represents one blank line)
        content.replace_range(span_start..span_end, "\n\n");
    }
}
