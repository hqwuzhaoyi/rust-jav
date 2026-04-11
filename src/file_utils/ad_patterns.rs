use regex::Regex;
use std::path::Path;

/// Load the repository's embedded ad/spam filename patterns.
pub fn embedded_patterns() -> Vec<String> {
    include_str!("../../patterns.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Convert a glob-like pattern (where `*` means “any run of characters”) into a
/// case-insensitive regex that matches the full filename. Literal segments are escaped so
/// dots and other regex metacharacters keep their literal meaning.
pub fn glob_to_regex(pattern: &str) -> Regex {
    let regex_str = pattern
        .split('*')
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(".*");
    Regex::new(&format!("(?i)^{regex_str}$")).expect("valid ad-pattern regex")
}

/// Compile pattern strings once for repeated filename checks.
pub fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter(|pattern| !pattern.trim().is_empty())
        .map(|pattern| glob_to_regex(pattern))
        .collect()
}

/// Return true when the filename matches at least one pattern.
pub fn filename_matches_any(filename: &str, patterns: &[String]) -> bool {
    let regexes = compile_patterns(patterns);
    filename_matches_any_compiled(filename, &regexes)
}

/// Return true when the filename matches at least one precompiled pattern.
pub fn filename_matches_any_compiled(filename: &str, regexes: &[Regex]) -> bool {
    regexes.iter().any(|regex| regex.is_match(filename))
}

/// Return true when the path's basename matches at least one pattern.
pub fn path_matches_any(path: &Path, patterns: &[String]) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|filename| filename_matches_any(filename, patterns))
}
