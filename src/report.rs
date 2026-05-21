use crate::rules::{Category, Match, MatchKind, Rule};
use owo_colors::OwoColorize;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct JsonReport {
    pub files: Vec<JsonFileReport>,
    pub summary: JsonSummary,
}

#[derive(Serialize)]
pub struct JsonFileReport {
    pub path: String,
    pub score: JsonScore,
    pub matches: Vec<Match>,
}

#[derive(Serialize)]
pub struct JsonScore {
    pub density: f32,
    pub total_matches: usize,
}

#[derive(Serialize)]
pub struct JsonSummary {
    pub total_files_scanned: usize,
    pub files_with_matches: usize,
    pub total_matches: usize,
    pub worst_rule: String,
}

pub fn print_human_report(
    file_path: &str,
    matches: &[Match],
    explain: bool,
) {
    if matches.is_empty() {
        return;
    }

    println!("{}:", file_path.bold());
    for m in matches {
        let cat_str = format!("[{}]", m.category);
        let conf_str = format!("[{:.2}]", m.confidence);
        
        let action_str = match &m.kind {
            MatchKind::Replace(s) => {
                if s.is_empty() {
                    format!("\"{}\" → (deleted)", m.original.red())
                } else {
                    format!("\"{}\" → \"{}\"", m.original.red(), s.green())
                }
            }
            MatchKind::DeleteLine | MatchKind::DeleteBlock => {
                format!("{} → (deleted)", m.original.trim().red())
            }
            MatchKind::FlagOnly => {
                format!("\"{}\" (flagged)", m.original.yellow())
            }
        };

        println!(
            "  {}:{}  {:<20} {:<15} {:<6} {}",
            m.line,
            m.col,
            m.rule_id.blue(),
            cat_str.dimmed(),
            conf_str.dimmed(),
            action_str
        );

        if explain {
            println!("        {}", m.description.italic());
        }
    }
    println!();
}

// ── Sentence-level scoring (Feature 2) ───────────────────────────────────────

/// A sentence with its AI-slop density score.
#[derive(Debug, Clone)]
pub struct SentenceScore {
    /// The sentence text (possibly truncated for display).
    pub text: String,
    /// 1-indexed line where the sentence starts.
    pub line: usize,
    /// Number of slop matches inside this sentence.
    pub hits: usize,
    /// hits / word_count — higher = denser slop.
    pub density: f32,
    /// The matched words/phrases inside this sentence.
    pub matched_words: Vec<String>,
}

/// Split `content` into sentences, returning `(sentence_text, start_byte, start_line)`.
fn split_sentences(content: &str) -> Vec<(String, usize, usize)> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut current_start = 0usize;
    let mut current_line = 1usize;
    let mut byte_offset = 0usize;
    let mut line = 1usize;

    for ch in content.chars() {
        if ch == '\n' {
            line += 1;
        }
        current.push(ch);
        byte_offset += ch.len_utf8();

        let ends_sentence = matches!(ch, '.' | '!' | '?');
        let next_is_space_or_end = content[byte_offset..]
            .chars()
            .next()
            .map_or(true, |c| c.is_whitespace());

        if ends_sentence && next_is_space_or_end && !current.trim().is_empty() {
            let trimmed = current.trim().to_string();
            sentences.push((trimmed, current_start, current_line));
            current_start = byte_offset;
            current_line = line;
            current.clear();
        }
    }

    // Flush remaining text
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push((trimmed, current_start, current_line));
    }

    sentences
}

/// Compute per-sentence AI-hit density for a single file's matches.
pub fn compute_sentence_density(content: &str, matches: &[Match]) -> Vec<SentenceScore> {
    let sentences = split_sentences(content);
    let mut scores = Vec::new();

    for (text, start_byte, line) in &sentences {
        let sent_len = text.len();
        let sent_end = start_byte + sent_len + 4; // small lookahead

        let sentence_matches: Vec<&Match> = matches
            .iter()
            .filter(|m| m.byte_range.start >= *start_byte && m.byte_range.start < sent_end)
            .collect();

        let hits = sentence_matches.len();
        if hits == 0 {
            continue;
        }

        let word_count = text.split_whitespace().count().max(1);
        let density = hits as f32 / word_count as f32;

        let matched_words: Vec<String> = sentence_matches
            .iter()
            .map(|m| m.original.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() < 60)
            .collect();

        scores.push(SentenceScore {
            text: if text.len() > 100 {
                format!("{}…", &text[..97])
            } else {
                text.clone()
            },
            line: *line,
            hits,
            density,
            matched_words,
        });
    }

    // Descending density
    scores.sort_by(|a, b| b.density.partial_cmp(&a.density).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Print the top-N sentences by AI hit density across all files.
pub fn print_sentence_report(file_results: &[(String, &str, Vec<Match>)], top_n: usize) {
    println!("Worst sentences by AI hit density");
    println!("─────────────────────────────────────────────────────");

    let mut all: Vec<(String, SentenceScore)> = Vec::new();
    for (path, content, matches) in file_results {
        for s in compute_sentence_density(content, matches) {
            all.push((path.clone(), s));
        }
    }

    all.sort_by(|a, b| b.1.density.partial_cmp(&a.1.density).unwrap_or(std::cmp::Ordering::Equal));

    if all.is_empty() {
        println!("  (no sentences with slop matches)");
        println!();
        return;
    }

    for (path, score) in all.iter().take(top_n) {
        println!(
            "  {} hits  density {:.2}  {}:{}",
            score.hits.to_string().yellow(),
            score.density,
            path,
            score.line,
        );
        println!("  \"{}\"", score.text.italic());
        if !score.matched_words.is_empty() {
            println!("  Matched: {}", score.matched_words.join(", ").red());
        }
        println!();
    }
}

// ── Summary report with coverage stats (Features 5) ─────────────────────────

pub fn print_summary(
    results: &[(String, Vec<Match>)],
    _total_files_scanned: usize,
    file_char_counts: &HashMap<String, usize>,
) {
    println!("Slop score report");
    println!("─────────────────────────────────────────────────────");

    let mut total_matches = 0;
    let mut files_with_matches = 0;
    let mut rule_counts: HashMap<String, usize> = HashMap::new();

    let mut file_scores = Vec::new();

    for (path, matches) in results {
        if matches.is_empty() {
            file_scores.push((path.clone(), 0.0, 0, String::new()));
            continue;
        }

        files_with_matches += 1;
        total_matches += matches.len();

        let mut cat_counts: HashMap<Category, usize> = HashMap::new();
        for m in matches {
            *cat_counts.entry(m.category).or_insert(0) += 1;
            *rule_counts.entry(m.rule_id.clone()).or_insert(0) += 1;
        }

        let char_count = file_char_counts.get(path).copied().unwrap_or(0);
        let density = if char_count > 0 {
            (matches.len() as f32 / char_count as f32) * 1000.0
        } else {
            0.0
        };

        let mut cat_strs = Vec::new();
        for (cat, count) in cat_counts {
            cat_strs.push(format!("{}:{}", cat, count));
        }
        cat_strs.sort();

        file_scores.push((path.clone(), density, matches.len(), cat_strs.join(", ")));
    }

    file_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (path, density, count, cats) in file_scores {
        if count == 0 {
            println!(" {:4.2} hits/1k  {:<20} clean", density, path);
        } else {
            println!(
                " {:4.2} hits/1k  {:<20} {} hits  {}",
                density.yellow(),
                path,
                count,
                cats.dimmed()
            );
        }
    }

    println!();
    println!(
        "Total: {} files with matches, {} total hits",
        files_with_matches, total_matches
    );

    if let Some((worst_rule, worst_count)) = rule_counts.iter().max_by_key(|(_, v)| *v) {
        println!(
            "Worst rule: {} ({} hits)",
            worst_rule.red(),
            worst_count
        );
    }
}

/// Print rule coverage statistics — which rules fired zero times.
pub fn print_coverage_stats(all_rule_ids: &[String], rule_counts: &HashMap<String, usize>) {
    let never_fired: Vec<&str> = all_rule_ids
        .iter()
        .filter(|id| !rule_counts.contains_key(*id))
        .map(|s| s.as_str())
        .collect();

    println!();
    if never_fired.is_empty() {
        println!("Rule coverage: all {} active rules fired.", all_rule_ids.len());
    } else {
        println!(
            "Rules that never fired ({} of {} active):",
            never_fired.len(),
            all_rule_ids.len()
        );
        for id in &never_fired {
            println!("  {}", id.dimmed());
        }
    }
    println!();
}

// ── explain-rules output (Feature 7) ────────────────────────────────────────

/// Print the explanation table for all active rules.
pub fn print_explain_rules(rules: &[Box<dyn Rule>]) {
    println!("Active deslop rules");
    println!("═══════════════════════════════════════════════════════════════════════");

    for rule in rules {
        let exp = rule.explain();
        println!();
        println!(
            "  {} [{}]  confidence: {:.2}",
            rule.id().bold().blue(),
            rule.category().to_string().dimmed(),
            exp.confidence,
        );
        println!("  Pattern:  {}", exp.pattern_summary);
        println!("  Fix:      {}", exp.fix_behavior);
        println!("  Example:  {} → {}", exp.example_input.red(), exp.example_output.green());
    }
    println!();
}
