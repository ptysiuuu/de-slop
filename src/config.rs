use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::rules::Profile;

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Config {
    pub profile: Option<Profile>,
    pub report: Option<bool>,
    pub min_confidence: Option<f32>,
    pub format: Option<String>,
    pub rules: Option<RulesConfig>,
    pub files: Option<FilesConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct RulesConfig {
    pub skip: Option<Vec<String>>,
    pub custom_phrases: Option<Vec<String>>,
    pub allow_symbols: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct FilesConfig {
    pub exclude: Option<Vec<String>>,
}

pub fn load_config(explicit_path: Option<&str>) -> Result<Option<(Config, PathBuf)>> {
    if let Some(path) = explicit_path {
        let p = Path::new(path);
        if p.exists() {
            let content = fs::read_to_string(p).context("Failed to read config file")?;
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            let config: Config = if ext == "yaml" || ext == "yml" {
                serde_yaml::from_str(&content).context("Failed to parse YAML config")?
            } else {
                toml::from_str(&content).context("Failed to parse TOML config")?
            };
            return Ok(Some((config, p.to_path_buf())));
        }
        anyhow::bail!("Config file not found: {}", path);
    }

    let mut current_dir = env::current_dir()?;
    loop {
        for filename in &[".desloprc.toml", ".desloprc.yaml", ".desloprc.yml"] {
            let p = current_dir.join(filename);
            if p.exists() {
                let content = fs::read_to_string(&p).context("Failed to read config file")?;
                let config: Config = if filename.ends_with(".yaml") || filename.ends_with(".yml") {
                    serde_yaml::from_str(&content).context("Failed to parse YAML config")?
                } else {
                    toml::from_str(&content).context("Failed to parse TOML config")?
                };
                return Ok(Some((config, p)));
            }
        }
        if !current_dir.pop() {
            break;
        }
    }
    Ok(None)
}
