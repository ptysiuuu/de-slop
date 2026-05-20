use crate::rules::{Category, Match, MatchKind};
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

    if let Some((worst_rule, worst_count)) = rule_counts.into_iter().max_by_key(|(_, v)| *v) {
        println!(
            "Worst rule: {} ({} hits)",
            worst_rule.red(),
            worst_count
        );
    }
}
