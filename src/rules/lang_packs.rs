//! Language-specific rule packs.
//!
//! Rules here have `Category::PackSpecific` and only fire when the active
//! profile is `rust-pack` or `python-pack` (or when `--extra-profile` stacks
//! one on top of a base profile). All rules are always registered in the
//! registry; `Profile::allows()` gates them.

use super::{line_col_from_offset, Category, FileTypeFilter, Match, MatchKind, Rule, RuleExplanation, Severity, Span};
use once_cell::sync::Lazy;
use regex::Regex;

// ── Rust pack ────────────────────────────────────────────────────────────────

/// Detects GPT-style Rust doc comments that just restate what the function
/// does without adding semantic information.
///
/// Patterns:
/// - "This function …" / "This method …" / "This struct …"
/// - "Returns the …" when it doesn't add type info
pub struct RustVerboseDocRule;

static RUST_VERBOSE_DOC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?im)^[ \t]*///[ \t]*(This (?:function|method|struct|enum|trait|impl|module) [a-zA-Z0-9_\s,]+\.|Returns? (?:the|a|an) [a-zA-Z0-9_\s]+\.)[ \t]*$"
    ).unwrap()
});

impl Rule for RustVerboseDocRule {
    fn id(&self) -> &str {
        "rust-verbose-doc"
    }

    fn category(&self) -> Category {
        Category::PackSpecific
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in RUST_VERBOSE_DOC.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::PackSpecific,
                severity: Severity::Warn,
                confidence: 0.80,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().trim().to_string(),
                suggestion: String::new(),
                kind: MatchKind::DeleteLine,
                description: "GPT-style Rust doc comment restates the obvious. Remove or replace with actual semantic information.".to_string(),
            });
        }
        matches
    }

    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Rust `///` doc comments starting with \"This function/method/struct…\" or \"Returns the/a…\"".to_string(),
            fix_behavior: "DeleteLine — removes the entire doc comment line".to_string(),
            confidence: 0.80,
            example_input: "/// This function processes the input data.".to_string(),
            example_output: "(line deleted)".to_string(),
        }
    }
}

/// Detects module-level doc comments that just repeat the module name.
pub struct RustTrivialModDocRule;

static RUST_MOD_DOC: Lazy<Regex> = Lazy::new(|| {
    // Matches: //! This module contains / //! This module provides / //! The X module
    Regex::new(
        r"(?im)^[ \t]*//![ \t]*(This module (?:contains|provides|implements|defines|handles) .+\.|The [a-zA-Z0-9_]+ module\.)"
    ).unwrap()
});

impl Rule for RustTrivialModDocRule {
    fn id(&self) -> &str {
        "rust-trivial-mod-doc"
    }

    fn category(&self) -> Category {
        Category::PackSpecific
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in RUST_MOD_DOC.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::PackSpecific,
                severity: Severity::Warn,
                confidence: 0.75,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().trim().to_string(),
                suggestion: String::new(),
                kind: MatchKind::DeleteLine,
                description: "Module doc comment restates the module name. Add real documentation.".to_string(),
            });
        }
        matches
    }

    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Inner doc comments (`//!`) that say \"This module contains/provides…\"".to_string(),
            fix_behavior: "DeleteLine".to_string(),
            confidence: 0.75,
            example_input: "//! This module contains the authentication logic.".to_string(),
            example_output: "(line deleted)".to_string(),
        }
    }
}

/// Returns all Rust pack rules.
pub fn rust_pack_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(RustVerboseDocRule),
        Box::new(RustTrivialModDocRule),
    ]
}

// ── Python pack ───────────────────────────────────────────────────────────────

/// Detects NumPy/Google-style Python docstrings that contain only boilerplate
/// `Args:` / `Returns:` sections with no real content.
///
/// "No real content" = the section has entries where the description is a
/// single generic word like "data", "input", "output", "result", "value".
pub struct PythonArgsBoilerplateRule;

static PY_ARGS_BOILERPLATE: Lazy<Regex> = Lazy::new(|| {
    // Match NumPy Args section where entries have trivial descriptions
    Regex::new(
        r#"(?ms)"""[\s\S]*?Args:\s*\n(?:[ \t]+\w+(?:\s*:\s*(?:The|A|An)\s+(?:input|output|data|result|value|parameter|argument|return)[^.\n]*\.?)\s*\n)+[\s\S]*?""""#
    ).unwrap()
});

static PY_RETURNS_BOILERPLATE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?ms)"""[\s\S]*?Returns:\s*\n[ \t]+(?:The|A|An)\s+(?:output|result|data|value|processed\s+data|return\s+value)[^.\n]*\.?[\s\S]*?""""#
    ).unwrap()
});

impl Rule for PythonArgsBoilerplateRule {
    fn id(&self) -> &str {
        "python-args-boilerplate"
    }

    fn category(&self) -> Category {
        Category::PackSpecific
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        // Check for Args boilerplate
        for m in PY_ARGS_BOILERPLATE.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::PackSpecific,
                severity: Severity::Warn,
                confidence: 0.75,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str()[..m.as_str().len().min(80)].to_string() + "…",
                suggestion: String::new(),
                kind: MatchKind::FlagOnly,
                description: "Python docstring Args section contains only generic boilerplate with no real semantic information.".to_string(),
            });
        }
        // Check for Returns boilerplate
        for m in PY_RETURNS_BOILERPLATE.find_iter(content) {
            // Skip if already covered by args match
            let already = matches.iter().any(|existing| {
                existing.byte_range.start <= m.start() && existing.byte_range.end >= m.end()
            });
            if already { continue; }
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::PackSpecific,
                severity: Severity::Warn,
                confidence: 0.75,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str()[..m.as_str().len().min(80)].to_string() + "…",
                suggestion: String::new(),
                kind: MatchKind::FlagOnly,
                description: "Python docstring Returns section contains only generic boilerplate.".to_string(),
            });
        }
        matches
    }

    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "NumPy/Google-style Python docstrings where Args/Returns sections contain only generic descriptions like \"The input data\", \"The output result\"".to_string(),
            fix_behavior: "FlagOnly — highlighted in report and interactive mode, never auto-deleted".to_string(),
            confidence: 0.75,
            example_input: "Args:\n    data: The input data.\nReturns:\n    The output data.".to_string(),
            example_output: "(flagged for review)".to_string(),
        }
    }
}

/// Detects empty NumPy docstring section headers (Args:, Returns:, etc. with
/// no content below them).
pub struct PythonNumpyEmptySectionRule;

static PY_EMPTY_SECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^([ \t]*(?:Args|Returns|Raises|Yields|Notes|References|Examples|Attributes):\s*\n)(?:[ \t]*(?:Args|Returns|Raises|Yields|Notes|References|Examples|Attributes):|[ \t]*""")"#
    ).unwrap()
});

impl Rule for PythonNumpyEmptySectionRule {
    fn id(&self) -> &str {
        "python-empty-docstring-section"
    }

    fn category(&self) -> Category {
        Category::PackSpecific
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::CodeOnly
    }

    fn check(&self, content: &str, _spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for c in PY_EMPTY_SECTION.captures_iter(content) {
            let m = c.get(1).unwrap();
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::PackSpecific,
                severity: Severity::Warn,
                confidence: 0.90,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().trim().to_string(),
                suggestion: String::new(),
                kind: MatchKind::DeleteLine,
                description: "Empty NumPy docstring section header with no content. Remove it.".to_string(),
            });
        }
        matches
    }

    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "NumPy-style docstring section headers (Args:, Returns:, etc.) that have no content below them".to_string(),
            fix_behavior: "DeleteLine".to_string(),
            confidence: 0.90,
            example_input: "Args:\nReturns:\n    int".to_string(),
            example_output: "Returns:\n    int".to_string(),
        }
    }
}

/// Returns all Python pack rules.
pub fn python_pack_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(PythonArgsBoilerplateRule),
        Box::new(PythonNumpyEmptySectionRule),
    ]
}
