use std::fmt;
use std::path::Path;

use regex::Regex;
use serde::Deserialize;

use crate::file_utils::ad_patterns;

pub const SUPPORTED_RULE_SET_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct ActiveRuleSet {
    version: u32,
    enabled_patterns: Vec<String>,
    compiled_patterns: Vec<Regex>,
}

impl PartialEq for ActiveRuleSet {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.enabled_patterns == other.enabled_patterns
    }
}

impl Eq for ActiveRuleSet {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleDocument {
    version: u32,
    rules: Vec<DeletionRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionRule {
    pattern: String,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    #[allow(dead_code)]
    note: Option<String>,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug)]
pub enum ActiveRuleSetError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    UnsupportedVersion(u32),
    EmptyPattern(usize),
    UnconfirmedEmpty,
}

impl fmt::Display for ActiveRuleSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read rule set: {error}"),
            Self::Yaml(error) => write!(f, "invalid rule set YAML: {error}"),
            Self::UnsupportedVersion(version) => write!(
                f,
                "unsupported rule set version {version}; expected {SUPPORTED_RULE_SET_VERSION}"
            ),
            Self::EmptyPattern(index) => {
                write!(f, "deletion rule {} has an empty pattern", index + 1)
            }
            Self::UnconfirmedEmpty => write!(
                f,
                "empty rule set requires explicit --confirm-empty-rules confirmation"
            ),
        }
    }
}

impl std::error::Error for ActiveRuleSetError {}

impl From<std::io::Error> for ActiveRuleSetError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_yaml::Error> for ActiveRuleSetError {
    fn from(value: serde_yaml::Error) -> Self {
        Self::Yaml(value)
    }
}

impl ActiveRuleSet {
    pub fn from_yaml(yaml: &str, confirm_empty: bool) -> Result<Self, ActiveRuleSetError> {
        let document: RuleDocument = serde_yaml::from_str(yaml)?;
        if document.version != SUPPORTED_RULE_SET_VERSION {
            return Err(ActiveRuleSetError::UnsupportedVersion(document.version));
        }
        for (index, rule) in document.rules.iter().enumerate() {
            if rule.pattern.trim().is_empty() {
                return Err(ActiveRuleSetError::EmptyPattern(index));
            }
        }
        let enabled_patterns = document
            .rules
            .into_iter()
            .filter(|rule| rule.enabled)
            .map(|rule| rule.pattern)
            .collect::<Vec<_>>();
        if enabled_patterns.is_empty() && !confirm_empty {
            return Err(ActiveRuleSetError::UnconfirmedEmpty);
        }
        let compiled_patterns = ad_patterns::compile_patterns(&enabled_patterns);
        Ok(Self {
            version: document.version,
            enabled_patterns,
            compiled_patterns,
        })
    }

    pub fn load(path: &Path, confirm_empty: bool) -> Result<Self, ActiveRuleSetError> {
        Self::from_yaml(&std::fs::read_to_string(path)?, confirm_empty)
    }

    pub fn embedded() -> Self {
        Self::from_yaml(Self::embedded_yaml(), false)
            .expect("embedded Active Rule Set must be valid")
    }

    pub fn embedded_yaml() -> &'static str {
        include_str!("../rules.yaml")
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn enabled_patterns(&self) -> Vec<String> {
        self.enabled_patterns.clone()
    }

    pub fn matches_basename(&self, basename: &str) -> bool {
        ad_patterns::filename_matches_any_compiled(basename, &self.compiled_patterns)
    }

    /// Returns the first enabled Deletion Rule matching a basename. Rule order
    /// is significant and is preserved from the Active Rule Set document.
    pub fn matching_pattern(&self, basename: &str) -> Option<&str> {
        self.compiled_patterns
            .iter()
            .position(|pattern| pattern.is_match(basename))
            .map(|index| self.enabled_patterns[index].as_str())
    }

    pub(crate) fn compiled_patterns(&self) -> &[Regex] {
        &self.compiled_patterns
    }
}

#[derive(Debug, Clone)]
pub struct ActiveRuleSetStore {
    active: ActiveRuleSet,
}

impl ActiveRuleSetStore {
    pub fn new(active: ActiveRuleSet) -> Self {
        Self { active }
    }

    pub fn active(&self) -> &ActiveRuleSet {
        &self.active
    }

    pub fn activate_yaml(
        &mut self,
        yaml: &str,
        confirm_empty: bool,
    ) -> Result<(), ActiveRuleSetError> {
        let candidate = ActiveRuleSet::from_yaml(yaml, confirm_empty)?;
        self.active = candidate;
        Ok(())
    }
}
