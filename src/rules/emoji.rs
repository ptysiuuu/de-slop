use super::{line_col_from_offset, Category, Match, MatchKind, Rule, Severity, Span};

pub struct EmojiRule {
    allow_set: Vec<char>,
}

impl EmojiRule {
    pub fn new(allow_symbols: &[String]) -> Self {
        let mut allow_set = Vec::new();
        for s in allow_symbols {
            if let Some(ch) = s.chars().next() {
                allow_set.push(ch);
            }
        }
        Self { allow_set }
    }
}

impl Rule for EmojiRule {
    fn id(&self) -> &str {
        "emoji"
    }

    fn category(&self) -> Category {
        Category::Symbol
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for (i, ch) in content.char_indices() {
            if self.allow_set.contains(&ch) {
                continue;
            }

            let code = ch as u32;
            let is_emoji = (0x1F000..=0x1FAFF).contains(&code)
                || (0x2600..=0x27BF).contains(&code)
                || (0x2300..=0x23FF).contains(&code)
                || (0x2460..=0x24FF).contains(&code)
                || (0x25A0..=0x25FF).contains(&code);

            if is_emoji {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Symbol,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::Replace(String::new()),
                    description: "Emoji character unlikely to be intentionally typed".to_string(),
                });
            }
        }
        matches
    }
}

pub struct DecorativeUnicodeRule {
    allow_set: Vec<char>,
}

impl DecorativeUnicodeRule {
    pub fn new(allow_symbols: &[String]) -> Self {
        let mut allow_set = Vec::new();
        for s in allow_symbols {
            if let Some(ch) = s.chars().next() {
                allow_set.push(ch);
            }
        }
        Self { allow_set }
    }

    fn is_decorative(ch: char) -> bool {
        matches!(
            ch,
            '★' | '✓' | '✔' | '→' | '◆' | '▸' | '•' | '●' | '○' | '◯' | '■' | '□' | '▪' | '▫' | '▲' | '▼' | '◀' | '▶' | '♦' | '♣' | '♠' | '♥' | '※' | '†' | '‡' | '§' | '¶'
        )
    }
}

impl Rule for DecorativeUnicodeRule {
    fn id(&self) -> &str {
        "decorative-unicode"
    }

    fn category(&self) -> Category {
        Category::Symbol
    }

    fn check(&self, content: &str, _comment_spans: Option<&[Span]>) -> Vec<Match> {
        let mut matches = Vec::new();
        for (i, ch) in content.char_indices() {
            if self.allow_set.contains(&ch) {
                continue;
            }

            if Self::is_decorative(ch) {
                let (line, col) = line_col_from_offset(content, i);
                matches.push(Match {
                    rule_id: self.id().to_string(),
                    category: Category::Symbol,
                    severity: Severity::Warn,
                    confidence: 1.0,
                    byte_range: i..(i + ch.len_utf8()),
                    line,
                    col,
                    original: ch.to_string(),
                    suggestion: String::new(),
                    kind: MatchKind::Replace(String::new()),
                    description: "Decorative Unicode symbol is a common LLM artifact".to_string(),
                });
            }
        }
        matches
    }
}
