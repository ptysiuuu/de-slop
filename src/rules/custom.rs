use super::{Category, Match, MatchKind, Rule, Severity, Span, line_col_from_offset};

use anyhow::{bail, Context, Result};
use globset::{Glob, GlobMatcher};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Part 1: CustomPhraseRule — from .desloprc.toml custom_phrases
// ---------------------------------------------------------------------------

pub struct CustomPhraseRule {
    pattern: Regex,
}

impl CustomPhraseRule {
    pub fn new(phrases: &[String]) -> Result<Self> {
        if phrases.is_empty() {
            bail!("no custom phrases provided");
        }
        let escaped: Vec<String> = phrases
            .iter()
            .map(|p| format!(r"(?i)\b{}\b", regex::escape(p)))
            .collect();
        let joined = escaped.join("|");
        let pattern = Regex::new(&joined).context("failed to compile custom phrase regex")?;
        Ok(Self { pattern })
    }
}

impl Rule for CustomPhraseRule {
    fn id(&self) -> &str {
        "custom-phrases"
    }

    fn category(&self) -> Category {
        Category::Custom
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in self.pattern.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Custom,
                severity: Severity::Warn,
                confidence: 0.75,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().to_string(),
                suggestion: String::new(),
                kind: MatchKind::Replace(String::new()),
                description: "Custom phrase match from config".to_string(),
            });
        }
        matches
    }
}

// ---------------------------------------------------------------------------
// Part 2: .deslop-rules file format
// ---------------------------------------------------------------------------

/// What fix action to apply for a custom file rule.
#[derive(Debug, Clone)]
pub enum FixAction {
    Delete,
    DeleteLine,
    Replace(String),
    Flag,
}

/// A single parsed rule from a .deslop-rules file.
#[derive(Debug, Clone)]

pub struct ParsedRule {
    pub name: String,
    pub pattern: String,
    pub is_regex: bool,
    pub fix: FixAction,
    pub message: Option<String>,
    pub confidence: f32,
    pub file_globs: Vec<GlobMatcher>,
    pub severity: Severity,
    pub source_line: usize,
}

/// A disable directive from a .deslop-rules file.
#[derive(Debug, Clone)]
pub struct DisableDirective {
    pub rule_id: String,
}

/// A fully parsed .deslop-rules file.
#[derive(Debug)]

pub struct DeslopRulesFile {
    pub path: PathBuf,
    pub rules: Vec<ParsedRule>,
    pub disables: Vec<DisableDirective>,
}

/// A validation error found during check-rules.
#[derive(Debug)]

pub struct ValidationError {
    pub line: usize,
    pub message: String,
}

// Known field names for validation / typo detection.
const KNOWN_FIELDS: &[&str] = &["match", "fix", "message", "confidence", "files", "severity"];

// Regex metacharacters that distinguish a regex from a literal.
const REGEX_META: &[char] = &['|', '(', '[', '*', '+', '?', '^', '$', '.'];

/// Simple Levenshtein distance for typo suggestions.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let a_len = a_bytes.len();
    let b_len = b_bytes.len();

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0usize; b_len + 1];

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

/// Suggest the closest known field name if within edit distance 2.
fn suggest_field(unknown: &str) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;
    for &field in KNOWN_FIELDS {
        let dist = levenshtein(unknown, field);
        if dist <= 2 && (best.is_none() || dist < best.unwrap().1) {
            best = Some((field, dist));
        }
    }
    best.map(|(f, _)| f.to_string())
}

/// Check if a pattern string contains regex metacharacters.
fn is_regex_pattern(pattern: &str) -> bool {
    pattern.chars().any(|c| REGEX_META.contains(&c))
}

/// Strip surrounding quotes from a string value.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse a .deslop-rules file, collecting all errors instead of bailing on the first.
pub fn parse_deslop_rules(path: &Path) -> Result<DeslopRulesFile> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut rules = Vec::new();
    let mut disables = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        if trimmed.starts_with("disable ") {
            let rule_id = trimmed.strip_prefix("disable ").unwrap().trim().to_string();
            if rule_id.is_empty() {
                errors.push(format!("line {}: empty disable directive", i + 1));
            } else {
                disables.push(DisableDirective { rule_id });
            }
            i += 1;
            continue;
        }

        if trimmed.starts_with("rule ") {
            let name = trimmed.strip_prefix("rule ").unwrap().trim().to_string();
            let rule_start_line = i + 1;
            if name.is_empty() {
                errors.push(format!("line {}: rule has no name", i + 1));
                i += 1;
                continue;
            }
            i += 1;

            // Collect fields for this rule block
            let mut fields: Vec<(String, String, usize)> = Vec::new();
            while i < lines.len() {
                let field_line = lines[i];
                let field_trimmed = field_line.trim();

                if field_trimmed.is_empty() {
                    i += 1;
                    continue;
                }
                if field_trimmed.starts_with('#') {
                    i += 1;
                    continue;
                }
                if !field_line.starts_with(' ') && !field_line.starts_with('\t') {
                    break;
                }

                if let Some(colon_pos) = field_trimmed.find(':') {
                    let field_name = field_trimmed[..colon_pos].trim().to_string();
                    let mut field_value = field_trimmed[colon_pos + 1..].trim().to_string();
                    let line_num = i + 1;
                    i += 1;

                    if field_value.starts_with("\"\"\"") && !field_value[3..].contains("\"\"\"") {
                        let mut parts = vec![field_value];
                        while i < lines.len() {
                            let ml = lines[i].trim();
                            parts.push(ml.to_string());
                            i += 1;
                            if ml.contains("\"\"\"") {
                                break;
                            }
                        }
                        field_value = parts.join("\n");
                    }

                    fields.push((field_name, field_value, line_num));
                } else {
                    errors.push(format!(
                        "line {}: expected 'field: value', got: {}",
                        i + 1,
                        field_trimmed
                    ));
                    i += 1;
                }
            }

            // Extract fields, accumulating errors for this rule
            let mut match_pattern: Option<String> = None;
            let mut fix_action: Option<FixAction> = None;
            let mut message: Option<String> = None;
            let mut confidence: f32 = 0.75;
            let mut file_globs: Vec<GlobMatcher> = Vec::new();
            let mut severity = Severity::Warn;
            let mut rule_ok = true;

            for (field_name, field_value, line_num) in &fields {
                match field_name.as_str() {
                    "match" => {
                        let val = field_value.as_str();
                        if val.starts_with("\"\"\"") {
                            let start_idx = 3;
                            let end_idx = val.rfind("\"\"\"").unwrap_or(val.len());
                            let inner = if end_idx > start_idx { &val[start_idx..end_idx] } else { &val[start_idx..] };
                            let joined = inner.lines().map(|l| l.trim()).collect::<Vec<_>>().join("");
                            match_pattern = Some(joined);
                        } else {
                            match_pattern = Some(strip_quotes(val).to_string());
                        }
                    }
                    "fix" => {
                        let val = field_value.trim();
                        fix_action = Some(match val {
                            "delete" => FixAction::Delete,
                            "delete-line" => FixAction::DeleteLine,
                            "flag" => FixAction::Flag,
                            _ => FixAction::Replace(strip_quotes(val).to_string()),
                        });
                    }
                    "message" => {
                        message = Some(strip_quotes(field_value).to_string());
                    }
                    "confidence" => {
                        match field_value.parse::<f32>() {
                            Ok(v) if (0.0..=1.0).contains(&v) => confidence = v,
                            Ok(v) => {
                                errors.push(format!(
                                    "line {}: confidence must be between 0.0 and 1.0, got {}",
                                    line_num, v
                                ));
                            }
                            Err(_) => {
                                errors.push(format!(
                                    "line {}: invalid confidence value: {}",
                                    line_num, field_value
                                ));
                            }
                        }
                    }
                    "files" => {
                        let globs_str = strip_quotes(field_value);
                        for glob_str in globs_str.split(',') {
                            let glob_str = glob_str.trim();
                            if !glob_str.is_empty() {
                                match Glob::new(glob_str) {
                                    Ok(g) => file_globs.push(g.compile_matcher()),
                                    Err(e) => errors.push(format!(
                                        "line {}: invalid glob pattern '{}': {}",
                                        line_num, glob_str, e
                                    )),
                                }
                            }
                        }
                    }
                    "severity" => {
                        match field_value.trim() {
                            "warn" => severity = Severity::Warn,
                            "error" => severity = Severity::Error,
                            v => errors.push(format!(
                                "line {}: invalid severity: {} (expected 'warn' or 'error')",
                                line_num, v
                            )),
                        }
                    }
                    unknown => {
                        let suggestion = suggest_field(unknown);
                        let msg = if let Some(ref s) = suggestion {
                            format!(
                                "line {}: unknown field \"{}\" (did you mean \"{}\"?)",
                                line_num, unknown, s
                            )
                        } else {
                            format!("line {}: unknown field \"{}\"", line_num, unknown)
                        };
                        errors.push(msg);
                    }
                }
            }

            let pattern = match match_pattern {
                Some(p) => p,
                None => {
                    errors.push(format!(
                        "line {}: rule '{}' missing required 'match' field",
                        rule_start_line, name
                    ));
                    rule_ok = false;
                    String::new()
                }
            };

            let fix = match fix_action {
                Some(f) => f,
                None => {
                    errors.push(format!(
                        "line {}: rule '{}' missing required 'fix' field",
                        rule_start_line, name
                    ));
                    rule_ok = false;
                    FixAction::Flag
                }
            };

            if !rule_ok {
                continue;
            }

            let is_regex = is_regex_pattern(&pattern);
            if is_regex {
                if let Err(e) = Regex::new(&pattern) {
                    errors.push(format!(
                        "line {}: rule '{}' has invalid regex: {}",
                        rule_start_line, name, e
                    ));
                    continue;
                }
            }

            rules.push(ParsedRule {
                name,
                pattern,
                is_regex,
                fix,
                message,
                confidence,
                file_globs,
                severity,
                source_line: rule_start_line,
            });

            continue;
        }

        errors.push(format!("line {}: unexpected line: {}", i + 1, trimmed));
        i += 1;
    }

    if !errors.is_empty() {
        let header = format!(
            "{}: {} error{} found",
            path.display(),
            errors.len(),
            if errors.len() == 1 { "" } else { "s" }
        );
        let detail = errors.iter().map(|e| format!("  {}", e)).collect::<Vec<_>>().join("\n");
        anyhow::bail!("{}\n{}", header, detail);
    }

    Ok(DeslopRulesFile {
        path: path.to_path_buf(),
        rules,
        disables,
    })
}

/// Discover all .deslop-rules files by walking up from start_dir.
pub fn discover_deslop_rules(start_dir: &Path) -> Result<Vec<DeslopRulesFile>> {
    let mut files = Vec::new();
    let mut dir = start_dir.to_path_buf();

    loop {
        let rules_path = dir.join(".deslop-rules");
        if rules_path.exists() {
            let parsed = parse_deslop_rules(&rules_path)?;
            files.push(parsed);
        }
        if !dir.pop() {
            break;
        }
    }

    // Deepest first (they were added starting from start_dir going up)
    // files[0] is the deepest, which is already correct
    Ok(files)
}

/// Merge all parsed rules. Deeper files override same-name rules from parent directories.
pub fn merge_rules(files: &[DeslopRulesFile]) -> (Vec<ParsedRule>, Vec<DisableDirective>) {
    let mut rule_map: HashMap<String, ParsedRule> = HashMap::new();
    let mut disables = Vec::new();

    // Process from shallowest to deepest so deeper overrides
    for file in files.iter().rev() {
        for rule in &file.rules {
            rule_map.insert(rule.name.clone(), rule.clone());
        }
        for disable in &file.disables {
            disables.push(disable.clone());
        }
    }

    (rule_map.into_values().collect(), disables)
}

// ---------------------------------------------------------------------------
// CustomFileRule — wraps a ParsedRule into a Rule impl
// ---------------------------------------------------------------------------

pub struct CustomFileRule {
    parsed: ParsedRule,
    compiled_pattern: Regex,
}


impl CustomFileRule {
    pub fn from_parsed(parsed: ParsedRule) -> Result<Self> {
        let compiled_pattern = if parsed.is_regex {
            Regex::new(&parsed.pattern)
                .with_context(|| format!("invalid regex in rule '{}': {}", parsed.name, parsed.pattern))?
        } else {
            // Literal match, case-insensitive, escaped
            Regex::new(&format!(r"(?i){}", regex::escape(&parsed.pattern)))
                .with_context(|| format!("failed to compile literal pattern for rule '{}'", parsed.name))?
        };
        Ok(Self {
            parsed,
            compiled_pattern,
        })
    }

    /// Check if this rule's file globs match the given path.
    pub fn matches_path(&self, path: &Path) -> bool {
        if self.parsed.file_globs.is_empty() {
            return true;
        }
        let path_str = path.to_string_lossy();
        self.parsed.file_globs.iter().any(|g| g.is_match(path_str.as_ref()))
    }
}

impl Rule for CustomFileRule {
    fn id(&self) -> &str {
        &self.parsed.name
    }

    fn category(&self) -> Category {
        Category::Custom
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();

        for m in self.compiled_pattern.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            let original = m.as_str().to_string();

            let (kind, suggestion) = match &self.parsed.fix {
                FixAction::Delete => (MatchKind::Replace(String::new()), String::new()),
                FixAction::DeleteLine => (MatchKind::DeleteLine, String::new()),
                FixAction::Replace(replacement) => {
                    // Handle capture group references ($1, $2, etc.)
                    let expanded = if self.parsed.is_regex {
                        // Re-run the regex to get captures for this specific match
                        if let Some(caps) = self.compiled_pattern.captures(&content[m.start()..]) {
                            let mut result = replacement.clone();
                            for i in (1..=9).rev() {
                                let placeholder = format!("${}", i);
                                if let Some(cap) = caps.get(i) {
                                    result = result.replace(&placeholder, cap.as_str());
                                }
                            }
                            result
                        } else {
                            replacement.clone()
                        }
                    } else {
                        replacement.clone()
                    };
                    (MatchKind::Replace(expanded.clone()), expanded)
                }
                FixAction::Flag => (MatchKind::FlagOnly, String::new()),
            };

            let description = self
                .parsed
                .message
                .clone()
                .unwrap_or_else(|| format!("Custom rule '{}' matched", self.parsed.name));

            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Custom,
                severity: self.parsed.severity,
                confidence: self.parsed.confidence,
                byte_range: m.start()..m.end(),
                line,
                col,
                original,
                suggestion,
                kind,
                description,
            });
        }

        matches
    }
}

// ---------------------------------------------------------------------------
// Validation for check-rules subcommand
// ---------------------------------------------------------------------------

/// Validate a .deslop-rules file and return all errors found.
pub fn validate_deslop_rules(path: &Path) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            errors.push(ValidationError {
                line: 0,
                message: format!("failed to read file: {}", e),
            });
            return errors;
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_rule: Option<(String, usize)> = None;
    let mut has_match = false;
    let mut has_fix = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        if trimmed.starts_with("disable ") {
            let rule_id = trimmed.strip_prefix("disable ").unwrap().trim();
            if rule_id.is_empty() {
                errors.push(ValidationError {
                    line: i + 1,
                    message: "empty disable directive".to_string(),
                });
            }
            i += 1;
            continue;
        }

        if trimmed.starts_with("rule ") {
            // Check previous rule had required fields
            if let Some((ref name, start_line)) = current_rule {
                if !has_match {
                    errors.push(ValidationError {
                        line: start_line,
                        message: format!("rule '{}' missing required 'match' field", name),
                    });
                }
                if !has_fix {
                    errors.push(ValidationError {
                        line: start_line,
                        message: format!("rule '{}' missing required 'fix' field", name),
                    });
                }
            }

            let name = trimmed.strip_prefix("rule ").unwrap().trim().to_string();
            if name.is_empty() {
                errors.push(ValidationError {
                    line: i + 1,
                    message: "rule has no name".to_string(),
                });
            }
            current_rule = Some((name, i + 1));
            has_match = false;
            has_fix = false;
            i += 1;
            continue;
        }

        // Indented field line
        if (line.starts_with(' ') || line.starts_with('\t')) && current_rule.is_some() {
            if let Some(colon_pos) = trimmed.find(':') {
                let field_name = trimmed[..colon_pos].trim();
                let field_value = trimmed[colon_pos + 1..].trim();

                match field_name {
                    "match" => {
                        has_match = true;
                        // Handle multiline
                        if field_value.starts_with("\"\"\"") {
                            let mut found_end = false;
                            let temp_i = i + 1;
                            let mut j = temp_i;
                            while j < lines.len() {
                                if lines[j].trim().contains("\"\"\"") {
                                    found_end = true;
                                    i = j;
                                    break;
                                }
                                j += 1;
                            }
                            if !found_end {
                                errors.push(ValidationError {
                                    line: i + 1,
                                    message: "unclosed multiline pattern (missing closing \"\"\")".to_string(),
                                });
                            }
                        } else {
                            let pattern = strip_quotes(field_value);
                            if is_regex_pattern(pattern) {
                                if let Err(e) = Regex::new(pattern) {
                                    errors.push(ValidationError {
                                        line: i + 1,
                                        message: format!("invalid regex: {}", e),
                                    });
                                }
                            }
                        }
                    }
                    "fix" => {
                        has_fix = true;
                        let val = field_value.trim();
                        if val != "delete"
                            && val != "delete-line"
                            && val != "flag"
                            && !(val.starts_with('"') && val.ends_with('"'))
                            && !(val.starts_with('\'') && val.ends_with('\''))
                        {
                            errors.push(ValidationError {
                                line: i + 1,
                                message: format!(
                                    "invalid fix value: {} (expected delete, delete-line, flag, or a quoted string)",
                                    val
                                ),
                            });
                        }
                    }
                    "message" => {}
                    "confidence" => {
                        if field_value.parse::<f32>().is_err() {
                            errors.push(ValidationError {
                                line: i + 1,
                                message: format!("invalid confidence value: {}", field_value),
                            });
                        } else {
                            let val: f32 = field_value.parse().unwrap_or(0.0);
                            if !(0.0..=1.0).contains(&val) {
                                errors.push(ValidationError {
                                    line: i + 1,
                                    message: format!(
                                        "confidence must be between 0.0 and 1.0, got {}",
                                        val
                                    ),
                                });
                            }
                        }
                    }
                    "files" => {
                        let globs_str = strip_quotes(field_value);
                        for glob_str in globs_str.split(',') {
                            let glob_str = glob_str.trim();
                            if !glob_str.is_empty() {
                                if let Err(e) = Glob::new(glob_str) {
                                    errors.push(ValidationError {
                                        line: i + 1,
                                        message: format!("invalid glob pattern '{}': {}", glob_str, e),
                                    });
                                }
                            }
                        }
                    }
                    "severity" => {
                        let val = field_value.trim();
                        if val != "warn" && val != "error" {
                            errors.push(ValidationError {
                                line: i + 1,
                                message: format!(
                                    "invalid severity: {} (expected 'warn' or 'error')",
                                    val
                                ),
                            });
                        }
                    }
                    unknown => {
                        let suggestion = suggest_field(unknown);
                        let msg = if let Some(ref s) = suggestion {
                            format!(
                                "unknown field \"{}\" (did you mean \"{}\"?)",
                                unknown, s
                            )
                        } else {
                            format!("unknown field \"{}\"", unknown)
                        };
                        errors.push(ValidationError {
                            line: i + 1,
                            message: msg,
                        });
                    }
                }
            } else {
                errors.push(ValidationError {
                    line: i + 1,
                    message: format!("expected 'field: value', got: {}", trimmed),

                });
            }
            i += 1;
            continue;
        }

        // Unrecognized line
        errors.push(ValidationError {
            line: i + 1,
            message: format!("unexpected line: {}", trimmed),

        });
        i += 1;
    }

    // Check last rule had required fields
    if let Some((ref name, start_line)) = current_rule {
        if !has_match {
            errors.push(ValidationError {
                line: start_line,
                message: format!("rule '{}' missing required 'match' field", name),

            });
        }
        if !has_fix {
            errors.push(ValidationError {
                line: start_line,
                message: format!("rule '{}' missing required 'fix' field", name),

            });
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("severity", "sevrity"), 1);
        assert_eq!(levenshtein("match", "match"), 0);
        assert_eq!(levenshtein("fix", "fiz"), 1);
        assert_eq!(levenshtein("confidence", "confidenc"), 1);
        assert_eq!(levenshtein("xyz", "abc"), 3);
    }

    #[test]
    fn test_suggest_field() {
        assert_eq!(suggest_field("sevrity"), Some("severity".to_string()));
        assert_eq!(suggest_field("mesage"), Some("message".to_string()));
        assert_eq!(suggest_field("matcg"), Some("match".to_string()));
        assert_eq!(suggest_field("xyz"), None);
    }

    #[test]
    fn test_is_regex_pattern() {
        assert!(!is_regex_pattern("simple phrase"));
        assert!(is_regex_pattern("word|other"));
        assert!(is_regex_pattern("(group)"));
        assert!(is_regex_pattern("[abc]"));
        assert!(is_regex_pattern("word.*thing"));
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
        assert_eq!(strip_quotes("  \"hello\"  "), "hello");
    }

    #[test]
    fn test_custom_phrase_rule() {
        let rule = CustomPhraseRule::new(&[
            "as mentioned above".to_string(),
            "in this regard".to_string(),
        ])
        .unwrap();

        let content = "As mentioned above, the code works. In this regard, we agree.";
        let matches = rule.check(content, None);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].rule_id, "custom-phrases");
        assert_eq!(matches[0].category, Category::Custom);
    }
}
