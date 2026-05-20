mod cli;
mod config;
mod detector;
mod diff;
mod engine;
mod interactive;
mod report;
mod rules;
mod scanner;

use anyhow::Result;
use clap::Parser;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cli::{Cli, Commands, FormatArg};
use config::load_config;
use detector::detect_file_type;
use engine::process_file;
use report::{print_human_report, print_summary, JsonFileReport, JsonReport, JsonScore, JsonSummary};
use rules::{build_registry, custom, Profile, Rule};
use scanner::{is_binary, Scanner};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.init {
        let p = std::env::current_dir()?.join(".desloprc.toml");
        if p.exists() {
            println!("Config file already exists at {}", p.display());
            std::process::exit(0);
        }
        // Write a basic default
        fs::write(&p, "profile = \"prose\"\nreport = true\n")?;
        println!("Wrote config to {}", p.display());
        std::process::exit(0);
    }

    if let Some(Commands::InstallHook) = cli.command {
        let hook_path = Path::new(".git/hooks/pre-commit");
        if let Some(parent) = hook_path.parent() {
            if !parent.exists() {
                anyhow::bail!("Not a git repository");
            }
        }
        let content = "#!/bin/sh\ndeslop --staged --threshold 1.0 --format json --report\n";
        fs::write(hook_path, content)?;
        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(hook_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(hook_path, perms)?;
        }
        println!("Installed pre-commit hook");
        std::process::exit(0);
    }

    let explicit_config = cli.config.as_deref();
    let config_res = load_config(explicit_config)?;
    let (config, _) = config_res.unwrap_or_default();

    if let Some(Commands::CheckRules) = cli.command {
        // Discover .deslop-rules files by walking up from cwd
        let mut rules_paths = Vec::new();
        let mut dir = std::env::current_dir()?;
        loop {
            let rules_path = dir.join(".deslop-rules");
            if rules_path.exists() {
                rules_paths.push(rules_path);
            }
            if !dir.pop() {
                break;
            }
        }

        let mut has_errors = false;
        for rules_path in &rules_paths {
            let errors = custom::validate_deslop_rules(rules_path);
            if !errors.is_empty() {
                has_errors = true;
                println!("{}", rules_path.display());
                for e in errors {
                    println!("  ✗ line {}: {}", e.line, e.message);
                }
            } else {
                println!("  ✓ {} valid", rules_path.display());
            }
        }
        if has_errors {
            std::process::exit(2);
        }
        std::process::exit(0);
    }

    let profile: Profile = cli.profile.into();
    let profile = config.profile.unwrap_or(profile);

    let min_confidence = cli.min_confidence.or(config.min_confidence).unwrap_or(0.6);

    let allow_symbols = config
        .rules
        .as_ref()
        .and_then(|r| r.allow_symbols.clone())
        .unwrap_or_default();
    let custom_phrases = config
        .rules
        .as_ref()
        .and_then(|r| r.custom_phrases.clone())
        .unwrap_or_default();
    
    let mut built_rules = build_registry(&allow_symbols, &custom_phrases);

    // Filter by profile
    built_rules.retain(|r| profile.allows(r.category()));

    // Load custom .deslop-rules
    let custom_files = custom::discover_deslop_rules(&std::env::current_dir()?)?;
    let (parsed_rules, disables) = custom::merge_rules(&custom_files);
    for pr in parsed_rules {
        built_rules.push(Box::new(custom::CustomFileRule::from_parsed(pr)?));
    }

    let mut skip_rules = cli.skip.clone();
    if let Some(cfg_rules) = &config.rules {
        if let Some(cfg_skip) = &cfg_rules.skip {
            skip_rules.extend(cfg_skip.clone());
        }
    }
    for d in disables {
        skip_rules.push(d.rule_id);
    }

    let mut final_rules: Vec<Box<dyn Rule>> = Vec::new();
    for r in built_rules {
        if skip_rules.contains(&r.id().to_string()) {
            continue;
        }
        if !cli.fix.is_empty() && !cli.fix.contains(&r.id().to_string()) {
            continue;
        }
        final_rules.push(r);
    }

    let excludes = config
        .files
        .as_ref()
        .and_then(|f| f.exclude.clone())
        .unwrap_or_default();

    let path_str = cli.path.unwrap_or_else(|| ".".to_string());
    let scanner = Scanner::discover(Path::new(&path_str), cli.recursive, &excludes)?;

    // Processing variables
    let results = Mutex::new(Vec::new());
    let char_counts = Mutex::new(HashMap::new());

    scanner.files().par_iter().for_each(|path| {
        let content_bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => return,
        };

        if is_binary(&content_bytes) {
            return;
        }

        let content = String::from_utf8_lossy(&content_bytes).to_string();
        char_counts.lock().unwrap().insert(path.to_string_lossy().to_string(), content.chars().count());

        let ext = path.extension().and_then(|s| s.to_str());
        let file_type = detect_file_type(&path.to_string_lossy(), ext, cli.lang.as_deref());

        let (modified, matches) = process_file(&content, &final_rules, file_type, min_confidence, ext);

        if !matches.is_empty() {
            if cli.interactive && !cli.dry_run {
                // Interactive blocks, so we do it serially. For now just filter.
                // In a true parallel map we shouldn't block for TUI.
                // We'll handle interactive sequentially after if needed, 
                // but rayon requires us to collect first for interactive.
            }
            results.lock().unwrap().push((path.clone(), content, modified, matches));
        }
    });

    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));

    let mut exit_code = 0;
    let mut total_matches = 0;
    let mut worst_rule_counts = HashMap::new();

    let mut json_files = Vec::new();
    let mut simplified_results = Vec::new();

    for (path, original, mut modified, mut matches) in results {
        if cli.interactive && !cli.dry_run {
            matches = interactive::run_interactive(&path.to_string_lossy(), &original, matches.clone())?;
            let ext = path.extension().and_then(|s| s.to_str());
            let file_type = detect_file_type(&path.to_string_lossy(), ext, cli.lang.as_deref());
            let (new_mod, _new_matches) = engine::process_file(&original, &final_rules, file_type, min_confidence, ext);
            modified = new_mod;
        }

        total_matches += matches.len();
        for m in &matches {
            *worst_rule_counts.entry(m.rule_id.clone()).or_insert(0) += 1;
        }

        let path_str = path.to_string_lossy().to_string();
        
        simplified_results.push((path_str.clone(), matches.clone()));

        if cli.format == FormatArg::Human {
            print_human_report(&path_str, &matches, cli.explain);
            if cli.diff {
                let diff = diff::generate_diff(&original, &modified, &path_str);
                println!("{}", diff);
            }
        } else if cli.format == FormatArg::Json {
            let char_count = char_counts.lock().unwrap().get(&path_str).copied().unwrap_or(0);
            let density = if char_count > 0 {
                (matches.len() as f32 / char_count as f32) * 1000.0
            } else {
                0.0
            };
            json_files.push(JsonFileReport {
                path: path_str.clone(),
                score: JsonScore {
                    density,
                    total_matches: matches.len(),
                },
                matches: matches.clone(),
            });
        }

        if !cli.dry_run && !cli.diff && cli.format == FormatArg::Human && original != modified {
            let out_path = if let Some(ref out) = cli.output {
                PathBuf::from(out)
            } else {
                path.clone()
            };
            fs::write(out_path, modified)?;
        }
    }

    if cli.format == FormatArg::Json {
        let worst_rule = worst_rule_counts.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k).unwrap_or_default();
        let report = JsonReport {
            files: json_files,
            summary: JsonSummary {
                total_files_scanned: scanner.files().len(),
                files_with_matches: simplified_results.len(),
                total_matches,
                worst_rule,
            },
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if cli.report || config.report.unwrap_or(false) {
        print_summary(&simplified_results, scanner.files().len(), &char_counts.into_inner().unwrap());
    }

    if let Some(_t) = cli.threshold {
        if total_matches > 0 {
            exit_code = 1;
        }
    }

    std::process::exit(exit_code);
}
