use similar::TextDiff;

pub fn generate_diff(original: &str, modified: &str, file_path: &str) -> String {
    let diff = TextDiff::from_lines(original, modified);
    diff.unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", file_path), &format!("b/{}", file_path))
        .to_string()
}
