use super::{line_col_from_offset, Category, FileTypeFilter, Match, MatchKind, Rule, Severity, Span};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

const STOP_WORDS: &[&str] = &[
    "initialize", "set", "get", "create", "return", "loop", "iterate", "check", "make", "add", 
    "update", "increment", "decrement", "append", "print", "call", "define", "declare", "assign", 
    "store", "compute", "calculate",
    // Articles, prepositions, and common filler
    "the", "a", "an", "to", "for", "of", "in", "on", "at", "by", "is", "it", "this", "that",
    "with", "from", "and", "or", "not", "if", "then", "else", "new", "into", "as", "up",
];

static STOP_WORDS_SET: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    STOP_WORDS.iter().copied().collect()
});

static WORD_SPLIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\s_]+|[[:punct:]]+").unwrap());

/// Split a string on camelCase boundaries without regex lookbehind.
fn split_camel_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len() {
        if i > 0 && chars[i].is_uppercase() && chars[i - 1].is_lowercase() && !current.is_empty() {
            parts.push(current.to_lowercase());
            current.clear();
        }
        current.push(chars[i]);
    }
    if !current.is_empty() {
        parts.push(current.to_lowercase());
    }
    parts
}

fn tokenize_comment(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in WORD_SPLIT.split(text) {
        if word.is_empty() {
            continue;
        }
        let lower = word.to_lowercase();
        if !STOP_WORDS_SET.contains(lower.as_str()) {
            tokens.push(lower);
        }
    }
    tokens
}

fn tokenize_code(text: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for chunk in WORD_SPLIT.split(text) {
        if chunk.is_empty() {
            continue;
        }
        for sub in split_camel_case(chunk) {
            if !sub.is_empty() {
                tokens.insert(sub);
            }
        }
    }
    tokens
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("--") || trimmed.starts_with('%')
}

fn is_doc_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("///") || trimmed.starts_with("//!")
}

fn strip_comment_prefix(line: &str) -> &str {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("//") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix('#') {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("--") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix('%') {
        rest
    } else {
        trimmed
    }
}

pub struct TrivialCommentRule;

impl Rule for TrivialCommentRule {
    fn id(&self) -> &str {
        "trivial-comment"
    }

    fn category(&self) -> Category {
        Category::Code
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        
        let lines: Vec<&str> = content.lines().collect();
        let mut byte_offset = 0;

        for (i, line) in lines.iter().enumerate() {
            let line_len = line.len();
            let next_offset = byte_offset + line_len + 1; // +1 for \n

            if is_comment_line(line) {
                // Doc comments (/// or //!) describe the item below, not a restatement
                // of a code line. Skip the trivial check for them entirely.
                if is_doc_comment_line(line) {
                    byte_offset = next_offset;
                    continue;
                }

                // Find adjacent code line
                let mut adj_code = None;

                // Look ahead
                for look_line in lines.iter().skip(i + 1) {
                    let trimmed = look_line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !is_comment_line(look_line) {
                        adj_code = Some(*look_line);
                        break;
                    }
                }

                // If not found ahead, look behind
                if adj_code.is_none() {
                    for j in (0..i).rev() {
                        let look_line = lines[j];
                        let trimmed = look_line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if !is_comment_line(look_line) {
                            adj_code = Some(look_line);
                            break;
                        }
                    }
                }

                if let Some(code_line) = adj_code {
                    let comment_text = strip_comment_prefix(line);
                    let comment_tokens = tokenize_comment(comment_text);
                    let code_tokens = tokenize_code(code_line);

                    if comment_tokens.is_empty() {
                        let (l, col) = line_col_from_offset(content, byte_offset);
                        matches.push(Match {
                            rule_id: self.id().to_string(),
                            category: Category::Code,
                            severity: Severity::Warn,
                            confidence: 0.85,
                            byte_range: byte_offset..next_offset.min(content.len()),
                            line: l,
                            col,
                            original: line.to_string(),
                            suggestion: String::new(),
                            kind: MatchKind::DeleteLine,
                            description: "Trivial comment (stop words only).".to_string(),
                        });
                    } else {
                        let mut overlap_count = 0;
                        for token in &comment_tokens {
                            if code_tokens.contains(token) {
                                overlap_count += 1;
                            }
                        }

                        let ratio = overlap_count as f32 / comment_tokens.len() as f32;
                        if ratio >= 0.80 {
                            let conf = if ratio == 1.0 { 0.85 } else { 0.70 };
                            let (l, col) = line_col_from_offset(content, byte_offset);
                            matches.push(Match {
                                rule_id: self.id().to_string(),
                                category: Category::Code,
                                severity: Severity::Warn,
                                confidence: conf,
                                byte_range: byte_offset..next_offset.min(content.len()),
                                line: l,
                                col,
                                original: line.to_string(),
                                suggestion: String::new(),
                                kind: MatchKind::DeleteLine,
                                description: "Trivial comment restates code.".to_string(),
                            });
                        }
                    }
                }
            }

            byte_offset = next_offset;
        }

        matches
    }
}

static DOCSTRING_GENERIC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^\s*(This (?:function|method|class|struct) [a-zA-Z0-9_\s]+)\s*$|^\s*(Parameters:\s*input\.?\s*Returns:\s*output\.?)\s*$").unwrap()
});

static PY_DOCSTRING: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?s)"""(.*?)"""|'''(.*?)'''"#).unwrap());
static RS_DOCSTRING: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?sm)^(?:[ \t]*///.*(?:\n|$))+").unwrap());
static JS_DOCSTRING: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?sm)^[ \t]*/\*\*.*?\*/").unwrap());

pub struct DocstringFillerRule;

impl Default for DocstringFillerRule {
    fn default() -> Self {
        Self
    }
}

impl DocstringFillerRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for DocstringFillerRule {
    fn id(&self) -> &str {
        "docstring-filler"
    }

    fn category(&self) -> Category {
        Category::Code
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();

        // Very basic detection of generic docstrings.
        // Python
        for m in PY_DOCSTRING.find_iter(content) {
            let inner = m.as_str().trim_matches(|c| c == '"' || c == '\'');
            if DOCSTRING_GENERIC.is_match(inner.trim()) {
                let (line, col) = line_col_from_offset(content, m.start());
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Code,
                    severity: Severity::Warn,
                    confidence: 0.80,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: m.as_str().to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::DeleteBlock,
                    description: "Filler docstring restates the function signature without adding information".to_string(),
                });
            }
        }

        // Rust
        for m in RS_DOCSTRING.find_iter(content) {
            let inner = m.as_str().replace("///", "").trim().to_string();
            if DOCSTRING_GENERIC.is_match(&inner) {
                let (line, col) = line_col_from_offset(content, m.start());
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Code,
                    severity: Severity::Warn,
                    confidence: 0.80,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: m.as_str().to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::DeleteBlock,
                    description: "Filler docstring restates the function signature without adding information".to_string(),
                });
            }
        }

        // JS
        for m in JS_DOCSTRING.find_iter(content) {
            let inner = m.as_str().replace("/**", "").replace("*/", "").replace("*", "").trim().to_string();
            if DOCSTRING_GENERIC.is_match(&inner) {
                let (line, col) = line_col_from_offset(content, m.start());
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Code,
                    severity: Severity::Warn,
                    confidence: 0.80,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: m.as_str().to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::DeleteBlock,
                    description: "Filler docstring restates the function signature without adding information".to_string(),
                });
            }
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial_comment_exact_subset() {
        let rule = TrivialCommentRule;
        let content = "
        // Initialize the user list
        let mut userList = initialize_users();
        ";
        let matches = rule.check(content, None);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].confidence, 0.85);
    }

    #[test]
    fn test_trivial_comment_not_trivial() {
        let rule = TrivialCommentRule;
        let content = "
        // Use a BTreeMap to keep items sorted by timestamp
        let mut items = HashMap::new();
        ";
        let matches = rule.check(content, None);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_trivial_comment_stop_words_only() {
        let rule = TrivialCommentRule;
        let content = "
        // Initialize
        let mut a = 1;
        ";
        let matches = rule.check(content, None);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].confidence, 0.85);
    }

    #[test]
    fn test_standalone_comment() {
        let rule = TrivialCommentRule;
        let content = "
        // Initialize the user list
        ";
        let matches = rule.check(content, None);
        assert_eq!(matches.len(), 0); // No adjacent code line
    }
}
