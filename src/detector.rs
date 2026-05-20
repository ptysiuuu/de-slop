use crate::rules::Span;
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FileType {
    Code,
    Prose,
    Unknown,
}

pub fn is_code_file(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "c" | "cpp" | "h" | "hpp" | "go" | "php" | "rb" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml" | "json" | "sql" | "kt" | "swift" | "cs" | "scala"
    )
}

pub fn is_prose_file(ext: &str) -> bool {
    matches!(
        ext,
        "md" | "markdown" | "txt" | "rst" | "adoc" | "tex"
    )
}

pub fn detect_file_type(_path: &str, ext: Option<&str>, lang_flag: Option<&str>) -> FileType {
    if let Some(lang) = lang_flag {
        if is_code_file(lang) {
            return FileType::Code;
        }
        if is_prose_file(lang) {
            return FileType::Prose;
        }
    }

    if let Some(e) = ext {
        if is_code_file(e) {
            return FileType::Code;
        }
        if is_prose_file(e) {
            return FileType::Prose;
        }
    }

    FileType::Prose // Fallback
}

pub fn extract_comment_spans(content: &str, ext: Option<&str>) -> Vec<Span> {
    let mut spans = Vec::new();
    let ext = ext.unwrap_or("");

    // Extremely basic block comment extraction for prose rule filtering
    if matches!(ext, "rs" | "js" | "ts" | "java" | "c" | "cpp" | "go" | "php" | "swift" | "kt" | "cs" | "scala") {
        static BLOCK_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
        for m in BLOCK_COMMENT.find_iter(content) {
            spans.push(Span { start: m.start(), end: m.end() });
        }
        static LINE_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)//.*$").unwrap());
        for m in LINE_COMMENT.find_iter(content) {
            spans.push(Span { start: m.start(), end: m.end() });
        }
    } else if matches!(ext, "py" | "rb" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml") {
        static HASH_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)#.*$").unwrap());
        for m in HASH_COMMENT.find_iter(content) {
            spans.push(Span { start: m.start(), end: m.end() });
        }
        if ext == "py" {
            static PY_DOCSTRING: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?s)"""(.*?)"""|'''(.*?)'''"#).unwrap());
            for m in PY_DOCSTRING.find_iter(content) {
                spans.push(Span { start: m.start(), end: m.end() });
            }
        }
    } else if ext == "sql" || ext == "lua" {
        static DASH_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)--.*$").unwrap());
        for m in DASH_COMMENT.find_iter(content) {
            spans.push(Span { start: m.start(), end: m.end() });
        }
    } else if ext == "tex" {
        static PERCENT_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)%.*$").unwrap());
        for m in PERCENT_COMMENT.find_iter(content) {
            spans.push(Span { start: m.start(), end: m.end() });
        }
    }

    spans.sort_by_key(|s| s.start);
    
    // Merge overlapping spans
    let mut merged: Vec<Span> = Vec::new();
    for span in spans {
        if let Some(last) = merged.last_mut() {
            if span.start <= last.end {
                last.end = last.end.max(span.end);
            } else {
                merged.push(span);
            }
        } else {
            merged.push(span);
        }
    }

    merged
}
