use super::{line_col_from_offset, Category, Match, MatchKind, Rule, RuleExplanation, Severity, Span};
use memchr::memchr;

pub struct EmDashRule;

impl Rule for EmDashRule {
    fn id(&self) -> &str {
        "em-dash"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        // Em dash U+2014 is \u{2014}, utf-8: e2 80 94
        if memchr(b'\xe2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if ch == '\u{2014}' {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: " - ".to_string(),
                    kind: MatchKind::Replace(" - ".to_string()),
                    description: "Em dash (U+2014) is rarely typed by humans and is a strong LLM signal.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Em dash character (U+2014 \u{2014}) — almost never typed by humans".to_string(),
            fix_behavior: "Replace with \" - \" (space-hyphen-space)".to_string(),
            confidence: 1.0,
            example_input: "word \u{2014} word".to_string(),
            example_output: "word  -  word".to_string(),
        }
    }
}

pub struct EnDashRule;

impl Rule for EnDashRule {
    fn id(&self) -> &str {
        "en-dash"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        // En dash U+2013 is \u{2013}, utf-8: e2 80 93
        if memchr(b'\xe2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if ch == '\u{2013}' {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: " - ".to_string(),
                    kind: MatchKind::Replace(" - ".to_string()),
                    description: "En dash (U+2013) is a common LLM typographic artifact.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "En dash character (U+2013 \u{2013})".to_string(),
            fix_behavior: "Replace with \" - \"".to_string(),
            confidence: 1.0,
            example_input: "pages 10\u{2013}20".to_string(),
            example_output: "pages 10 - 20".to_string(),
        }
    }
}

pub struct CurlyQuotesRule;

impl Rule for CurlyQuotesRule {
    fn id(&self) -> &str {
        "curly-quotes"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        if memchr(b'\xe2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            let replacement = match ch {
                '\u{201C}' | '\u{201D}' => "\"",
                '\u{2018}' | '\u{2019}' => "'",
                _ => continue,
            };
            let (line, col) = line_col_from_offset(content, i);
            matches.push(Match {
                rule_id: self.id().to_string(),
                category: Category::Typographic,
                severity: Severity::Warn,
                confidence: 1.0,
                byte_range: i..(i + ch.len_utf8()),
                line,
                col,
                original: ch.to_string(),
                suggestion: replacement.to_string(),
                kind: MatchKind::Replace(replacement.to_string()),
                description: "Curly quotes are common LLM typographic artifacts.".to_string(),
            });
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Curly/smart quotes: \u{201C}\u{201D} (double) and \u{2018}\u{2019} (single)".to_string(),
            fix_behavior: "Replace with straight quotes \" or '".to_string(),
            confidence: 1.0,
            example_input: "\u{201C}hello\u{201D}".to_string(),
            example_output: "\"hello\"".to_string(),
        }
    }
}

pub struct EllipsisRule;

impl Rule for EllipsisRule {
    fn id(&self) -> &str {
        "ellipsis"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        if memchr(b'\xe2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if ch == '\u{2026}' {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: "...".to_string(),
                    kind: MatchKind::Replace("...".to_string()),
                    description: "Ellipsis character (U+2026) is a common LLM typographic artifact.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Ellipsis character U+2026 (\u{2026})".to_string(),
            fix_behavior: "Replace with three dots \"...\"".to_string(),
            confidence: 1.0,
            example_input: "and so on\u{2026}".to_string(),
            example_output: "and so on...".to_string(),
        }
    }
}

pub struct NbspRule;

impl Rule for NbspRule {
    fn id(&self) -> &str {
        "nbsp"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        // NBSP U+00A0 is utf-8: c2 a0
        if memchr(b'\xc2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if ch == '\u{00A0}' {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: " ".to_string(),
                    kind: MatchKind::Replace(" ".to_string()),
                    description: "Non-breaking space (U+00A0) is often generated by LLMs unnecessarily.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Non-breaking space (U+00A0)".to_string(),
            fix_behavior: "Replace with regular space".to_string(),
            confidence: 1.0,
            example_input: "word\u{00A0}word".to_string(),
            example_output: "word word".to_string(),
        }
    }
}

pub struct ZeroWidthRule;

impl Rule for ZeroWidthRule {
    fn id(&self) -> &str {
        "zero-width"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        if memchr(b'\xe2', content.as_bytes()).is_none() && memchr(b'\xef', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if matches!(ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}') {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::Replace(String::new()),
                    description: "Zero-width character is an invisible typographic artifact.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Zero-width characters: U+200B, U+200C, U+200D, U+FEFF".to_string(),
            fix_behavior: "Delete — removes the invisible character".to_string(),
            confidence: 1.0,
            example_input: "word\u{200B}word".to_string(),
            example_output: "wordword".to_string(),
        }
    }
}

pub struct SoftHyphenRule;

impl Rule for SoftHyphenRule {
    fn id(&self) -> &str {
        "soft-hyphen"
    }
    fn category(&self) -> Category {
        Category::Typographic
    }
    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        // Soft hyphen U+00AD utf-8: c2 ad
        if memchr(b'\xc2', content.as_bytes()).is_none() {
            return matches;
        }
        for (i, ch) in content.char_indices() {
            if ch == '\u{00AD}' {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Typographic,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::Replace(String::new()),
                    description: "Soft hyphen (U+00AD) is an invisible typographic artifact.".to_string(),
                });
            }
        }
        matches
    }
    fn explain(&self) -> RuleExplanation {
        RuleExplanation {
            pattern_summary: "Soft hyphen (U+00AD) — invisible optional hyphen".to_string(),
            fix_behavior: "Delete — removes the character".to_string(),
            confidence: 1.0,
            example_input: "soft\u{00AD}hyphen".to_string(),
            example_output: "softhyphen".to_string(),
        }
    }
}
