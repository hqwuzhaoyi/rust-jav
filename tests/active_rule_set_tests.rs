use rust_jav::active_rules::{ActiveRuleSet, ActiveRuleSetStore};

const VALID: &str = r#"
version: 1
rules:
  - pattern: "*.HTML"
    note: literal extension with case-insensitive matching
  - pattern: "disabled*.txt"
    enabled: false
"#;

#[test]
fn versioned_yaml_supports_required_pattern_optional_enabled_and_note() {
    let rules = ActiveRuleSet::from_yaml(VALID, false).unwrap();

    assert_eq!(rules.version(), 1);
    assert_eq!(rules.enabled_patterns(), vec!["*.HTML"]);
    assert!(rules.matches_basename("offer.html"));
    assert!(!rules.matches_basename("disabled-offer.txt"));
}

#[test]
fn invalid_candidate_does_not_replace_last_valid_active_rule_set() {
    let mut store = ActiveRuleSetStore::new(ActiveRuleSet::from_yaml(VALID, false).unwrap());

    let error = store.activate_yaml("version: 1\nrules:\n  - enabled: true\n", false);

    assert!(error.is_err());
    assert!(store.active().matches_basename("offer.html"));
}

#[test]
fn unsupported_version_is_invalid() {
    let error =
        ActiveRuleSet::from_yaml("version: 2\nrules: [{ pattern: '*' }]", false).unwrap_err();
    assert!(error.to_string().contains("version"));
}

#[test]
fn empty_rule_set_requires_explicit_confirmation() {
    let yaml = "version: 1\nrules: []\n";

    assert!(ActiveRuleSet::from_yaml(yaml, false).is_err());
    assert!(ActiveRuleSet::from_yaml(yaml, true).is_ok());
}

#[test]
fn embedded_yaml_preserves_every_legacy_pattern_during_migration() {
    let legacy = include_str!("../patterns.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    assert_eq!(ActiveRuleSet::embedded().enabled_patterns(), legacy);
}
