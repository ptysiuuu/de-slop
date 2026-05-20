use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Scanner {
    files: Vec<PathBuf>,
}

impl Scanner {
    pub fn discover(
        path: &Path,
        recursive: bool,
        excludes: &[String],
    ) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for ex in excludes {
            builder.add(Glob::new(ex)?);
        }
        let exclude_set = builder.build()?;

        let mut files = Vec::new();

        if path.is_file() {
            files.push(path.to_path_buf());
            return Ok(Self { files });
        }

        if !path.is_dir() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        let walker = WalkDir::new(path).min_depth(1);
        let walker = if recursive { walker } else { walker.max_depth(1) };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let p = entry.path();
                
                // Exclude using globset
                let path_str = p.to_string_lossy();
                if exclude_set.is_match(path_str.as_ref()) {
                    continue;
                }
                
                files.push(p.to_path_buf());
            }
        }

        Ok(Self { files })
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
}

pub fn is_binary(content: &[u8]) -> bool {
    memchr::memchr(b'\0', content).is_some()
}
