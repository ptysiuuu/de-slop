use super::{line_col_from_offset, Category, FileTypeFilter, Match, MatchKind, Rule, RuleExplanation, Severity, Span};
use once_cell::sync::Lazy;
use phf::phf_map;
use regex::Regex;

static FILLER_OPENERS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)(?:^|\n)\s*(?:Certainly|Absolutely|Of course|Great question|Sure|Definitely)!?\s*").unwrap()
});
static FILLER_OPENERS_2: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)(?:^|\n)\s*(?:I'?d be happy to|Happy to help)[.,!]?\s*").unwrap()
});

pub struct FillerOpenersRule;

impl Default for FillerOpenersRule {
    fn default() -> Self {
        Self
    }
}

impl FillerOpenersRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for FillerOpenersRule {
    fn id(&self) -> &str {
        "filler-openers"
    }

    fn category(&self) -> Category {
        Category::Prose
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for re in [&*FILLER_OPENERS, &*FILLER_OPENERS_2] {
            for m in re.find_iter(content) {
                let (line, col) = line_col_from_offset(content, m.start());
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Prose,
                    severity: Severity::Warn,
                    confidence: 0.95,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: m.as_str().to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::Replace(String::new()),
                    description: "Filler opener. State the point directly.".to_string(),
                });
            }
        }
        matches
    }

    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Phrases at sentence start: \"Certainly!\", \"Absolutely!\", \"Of course!\", \"I'd be happy to\"".to_string(),
            fix_behavior: "Replace(delete) — removes the filler phrase".to_string(),
            confidence: 0.95,
            example_input: "Certainly! Here is the solution.".to_string(),
            example_output: "Here is the solution.".to_string(),
        }
    }
}

static SYCOPHANTIC_CLOSERS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)\b(?:I hope this helps|Feel free to ask|Let me know if you have questions|Don't hesitate to reach out|Hope that clarifies things)[.,!]?\s*").unwrap()
});

pub struct SycophanticClosersRule;

impl Default for SycophanticClosersRule {
    fn default() -> Self {
        Self
    }
}

impl SycophanticClosersRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for SycophanticClosersRule {
    fn id(&self) -> &str {
        "sycophantic-closers"
    }

    fn category(&self) -> Category {
        Category::Prose
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in SYCOPHANTIC_CLOSERS.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Prose,
                severity: Severity::Warn,
                confidence: 0.90,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().to_string(),
                suggestion: String::new(),
                kind: MatchKind::Replace(String::new()),
                description: "Sycophantic closer. Delete the phrase.".to_string(),
            });
        }
        matches
    }
}

static HEDGES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)\b(?:it is worth noting that|it is important to remember that|it is important to note that|keep in mind that|note that|please note that|it should be noted that)\s*").unwrap()
});

pub struct HedgesRule;

impl Default for HedgesRule {
    fn default() -> Self {
        Self
    }
}

impl HedgesRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for HedgesRule {
    fn id(&self) -> &str {
        "hedges"
    }

    fn category(&self) -> Category {
        Category::Prose
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in HEDGES.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Prose,
                severity: Severity::Warn,
                confidence: 0.85,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().to_string(),
                suggestion: String::new(),
                kind: MatchKind::Replace(String::new()),
                description: "Hedge phrase. Delete it.".to_string(),
            });
        }
        matches
    }
}

static TRANSITION_PADDING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)(?:^|\n)\s*(?:In conclusion|To summarize|To recap|At the end of the day|In summary|All in all|Ultimately)[,\s]+").unwrap()
});

pub struct TransitionPaddingRule;

impl Default for TransitionPaddingRule {
    fn default() -> Self {
        Self
    }
}

impl TransitionPaddingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for TransitionPaddingRule {
    fn id(&self) -> &str {
        "transition-padding"
    }

    fn category(&self) -> Category {
        Category::Prose
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in TRANSITION_PADDING.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Prose,
                severity: Severity::Warn,
                confidence: 0.80,
                byte_range: m.start()..m.end(),
                line,
                col,
                original: m.as_str().to_string(),
                suggestion: String::new(),
                kind: MatchKind::Replace(String::new()),
                description: "Transitional padding. Delete the phrase.".to_string(),
            });
        }
        matches
    }
}

static SYNONYMS: phf::Map<&'static str, &'static str> = phf_map! {
    "utilize" => "use",
    "facilitate" => "help",
    "leverage" => "use",
    "implement" => "build",
    "commence" => "start",
    "endeavor" => "try",
    "terminate" => "end",
    "obtain" => "get",
    "regarding" => "about",
    "additional" => "more",
    "individuals" => "people",
    "methodology" => "method",
};

const FLAG_ONLY: &[&str] = &[
    "seamlessly", "robust", "straightforward", "scalable", "cutting-edge", 
    "state-of-the-art", "comprehensive", "streamline", "synergy", "paradigm", 
    "holistic", "proactive"
];

static HOLLOW_INTENSIFIERS: Lazy<Regex> = Lazy::new(|| {
    let mut words = Vec::new();
    for key in SYNONYMS.keys() {
        words.push(*key);
    }
    words.extend_from_slice(FLAG_ONLY);
    let pattern = format!(r"(?i)\b({})\b", words.join("|"));
    Regex::new(&pattern).unwrap()
});

pub struct HollowIntensifiersRule;

impl Rule for HollowIntensifiersRule {
    fn id(&self) -> &str {
        "hollow-intensifiers"
    }

    fn category(&self) -> Category {
        Category::Prose
    }

    fn file_type_filter(&self) -> FileTypeFilter {
        FileTypeFilter::All
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for m in HOLLOW_INTENSIFIERS.find_iter(content) {
            let (line, col) = line_col_from_offset(content, m.start());
            let word = m.as_str();
            let lower = word.to_lowercase();
            
            if let Some(&synonym) = SYNONYMS.get(lower.as_str()) {
                // Preserve casing
                let replacement = if word.chars().all(|c| c.is_uppercase()) {
                    synonym.to_uppercase()
                } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let mut chars = synonym.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().chain(chars).collect(),
                    }
                } else {
                    synonym.to_string()
                };

                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Prose,
                    severity: Severity::Warn,
                    confidence: 0.70,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: word.to_string(),
                    suggestion: replacement.clone(),
                    kind: MatchKind::Replace(replacement),
                    description: format!("Hollow intensifier. Use '{}' instead.", synonym),
                });
            } else {
                // Flag only
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Prose,
                    severity: Severity::Warn,
                    confidence: 0.70,
                    byte_range: m.start()..m.end(),
                    line,
                    col,
                    original: word.to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::FlagOnly,
                    description: "Hollow intensifier. Rephrase to be more direct.".to_string(),
                });
            }
        }
        matches
    }
}
