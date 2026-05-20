pub mod typography;
pub mod emoji;
pub mod prose;
pub mod code;
pub mod custom;

use std::fmt;
use std::ops::Range;

use serde::{Serialize, Deserialize};

/// Category of a rule — set by the rule at match creation time, never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Typographic,
    Symbol,
    Prose,
    Code,
    Custom,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Category::Typographic => write!(f, "typographic"),
            Category::Symbol => write!(f, "symbol"),
            Category::Prose => write!(f, "prose"),
            Category::Code => write!(f, "code"),
            Category::Custom => write!(f, "custom"),
        }
    }
}

/// Severity level for a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warn,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Warn => write!(f, "warn"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// What kind of fix this match represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum MatchKind {
    /// Replace the matched byte range with the given string.
    Replace(String),
    /// Delete the entire line containing the match, including its newline.
    DeleteLine,
    /// Delete an entire block (e.g., a docstring) from opening to closing delimiter.
    DeleteBlock,
    /// Flag only — shown in reports and interactive mode, never written to disk.
    FlagOnly,
}

impl fmt::Display for MatchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchKind::Replace(_) => write!(f, "Replace"),
            MatchKind::DeleteLine => write!(f, "DeleteLine"),
            MatchKind::DeleteBlock => write!(f, "DeleteBlock"),
            MatchKind::FlagOnly => write!(f, "FlagOnly"),
        }
    }
}

/// Whether a rule applies to all files, prose-only, or code-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTypeFilter {
    All,
    ProseOnly,
    CodeOnly,
}

/// A single detected pattern match.
#[derive(Debug, Clone, Serialize)]
pub struct Match {
    /// Stable rule identifier (e.g., "em-dash", "trivial-comment").
    pub rule_id: String,
    /// Category — set by the rule, never inferred.
    pub category: Category,
    /// Severity level.
    pub severity: Severity,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f32,
    /// Byte range in the original file content.
    #[serde(skip)]
    pub byte_range: Range<usize>,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column (char count).
    pub col: usize,
    /// The matched text.
    pub original: String,
    /// Replacement text (empty string = delete span only).
    pub suggestion: String,
    /// What kind of fix to apply.
    pub kind: MatchKind,
    /// Human-readable explanation.
    pub description: String,
}

/// A span within a file (e.g., a comment or docstring).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn contains_range(&self, range: &Range<usize>) -> bool {
        range.start >= self.start && range.end <= self.end
    }
}

/// The Rule trait — every detection rule implements this.
pub trait Rule: Send + Sync {
    /// Stable string ID for this rule (used by --fix and --skip).
    fn id(&self) -> &str;

    /// The category this rule belongs to.
    fn category(&self) -> Category;

    /// The file type filter for this rule.
    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    /// Run this rule against the given content and return all matches.
    /// `comment_spans` is provided for code files so rules can restrict to comments.
    fn check(&self, content: &str, comment_spans: Option<&[Span]>) -> Vec<Match>;
}

/// Compute 1-indexed line and column from a byte offset in content.
pub fn line_col_from_offset(content: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in content.char_indices() {
        if i >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Build the default registry of built-in rules.
pub fn build_registry(
    allow_symbols: &[String],
    custom_phrases: &[String],
) -> Vec<Box<dyn Rule>> {
    let mut rules: Vec<Box<dyn Rule>> = Vec::new();

    // Typographic rules (highest priority — lowest registry index)
    rules.push(Box::new(typography::EmDashRule));
    rules.push(Box::new(typography::EnDashRule));
    rules.push(Box::new(typography::CurlyQuotesRule));
    rules.push(Box::new(typography::EllipsisRule));
    rules.push(Box::new(typography::NbspRule));
    rules.push(Box::new(typography::ZeroWidthRule));
    rules.push(Box::new(typography::SoftHyphenRule));

    // Symbol rules
    rules.push(Box::new(emoji::EmojiRule::new(allow_symbols)));
    rules.push(Box::new(emoji::DecorativeUnicodeRule::new(allow_symbols)));

    // Prose rules
    rules.push(Box::new(prose::FillerOpenersRule::new()));
    rules.push(Box::new(prose::SycophanticClosersRule::new()));
    rules.push(Box::new(prose::HedgesRule::new()));
    rules.push(Box::new(prose::TransitionPaddingRule::new()));
    rules.push(Box::new(prose::HollowIntensifiersRule));

    // Code rules
    rules.push(Box::new(code::TrivialCommentRule));
    rules.push(Box::new(code::DocstringFillerRule::new()));

    // Custom phrase rules from config
    if !custom_phrases.is_empty() {
        if let Ok(rule) = custom::CustomPhraseRule::new(custom_phrases) {
            rules.push(Box::new(rule));
        }
    }

    rules
}

/// Profile determines which rule categories are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Conservative,
    Prose,
    Code,
    Aggressive,
}

impl Profile {
    /// Returns true if the given category is active under this profile.
    pub fn allows(&self, category: Category) -> bool {
        match self {
            Profile::Conservative => matches!(category, Category::Typographic | Category::Symbol | Category::Custom),
            Profile::Prose => matches!(category, Category::Typographic | Category::Symbol | Category::Prose | Category::Custom),
            Profile::Code => matches!(category, Category::Typographic | Category::Symbol | Category::Code | Category::Custom),
            Profile::Aggressive => true,
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Profile::Conservative => write!(f, "conservative"),
            Profile::Prose => write!(f, "prose"),
            Profile::Code => write!(f, "code"),
            Profile::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl std::str::FromStr for Profile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "conservative" => Ok(Profile::Conservative),
            "prose" => Ok(Profile::Prose),
            "code" => Ok(Profile::Code),
            "aggressive" => Ok(Profile::Aggressive),
            _ => Err(anyhow::anyhow!("unknown profile: {}", s)),
        }
    }
}
