use clap::Parser;

use rust_jav::cli::{Cli, Command};

#[test]
fn parses_serve_command_with_config_path() {
    let cli = Cli::try_parse_from(["rust-jav", "serve", "--config", "management.yaml"]).unwrap();

    match cli.command {
        Command::Serve(args) => assert_eq!(args.config.to_string_lossy(), "management.yaml"),
        other => panic!("expected serve command, got {other:?}"),
    }
}

#[test]
fn parses_local_administrator_commands() {
    for action in ["init", "reset-password"] {
        let cli = Cli::try_parse_from([
            "rust-jav",
            "administrator",
            action,
            "--config",
            "management.yaml",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Administrator(_)));
    }
}

#[test]
fn parses_tui_command() {
    let cli = Cli::try_parse_from(["rust-jav", "tui", "--dir", "./examples/test"]).unwrap();

    match cli.command {
        Command::Tui(args) => assert_eq!(args.dir.to_string_lossy(), "./examples/test"),
        other => panic!("expected tui command, got {other:?}"),
    }
}

#[test]
fn parses_ops_command_with_default_preview() {
    let cli = Cli::try_parse_from([
        "rust-jav",
        "ops",
        "--dir",
        "./examples/test",
        "--op",
        "standardize-names",
        "--json",
    ])
    .unwrap();

    match cli.command {
        Command::Ops(args) => {
            assert!(!args.apply, "ops should default to preview mode");
            assert!(args.json);
            assert_eq!(args.ops.len(), 1);
        }
        other => panic!("expected ops command, got {other:?}"),
    }
}

#[test]
fn parses_actor_links_apply_command() {
    let cli = Cli::try_parse_from([
        "rust-jav",
        "actor-links",
        "--source",
        "./examples/test",
        "--actors-root",
        "./actors",
        "--apply",
    ])
    .unwrap();

    match cli.command {
        Command::ActorLinks(args) => {
            assert!(args.apply);
            assert_eq!(args.source.to_string_lossy(), "./examples/test");
            assert_eq!(args.actors_root.to_string_lossy(), "./actors");
        }
        other => panic!("expected actor-links command, got {other:?}"),
    }
}

#[test]
fn parses_delete_ad_files_operation() {
    let cli = Cli::try_parse_from([
        "rust-jav",
        "ops",
        "--dir",
        "./examples/test",
        "--op",
        "delete-ad-files",
        "--json",
    ])
    .unwrap();

    match cli.command {
        Command::Ops(args) => {
            assert!(!args.apply, "should default to preview");
            assert_eq!(args.ops.len(), 1);
            let ops = args.selected_operations();
            assert_eq!(ops.len(), 1);
            assert_eq!(ops[0].name(), "Delete Ad Files");
        }
        other => panic!("expected ops command, got {other:?}"),
    }
}

#[test]
fn delete_ad_files_is_first_in_all_operations() {
    use rust_jav::tui::state::OperationType;
    let all = OperationType::all();
    assert_eq!(
        all[0],
        OperationType::DeleteAdFiles,
        "DeleteAdFiles must be first in OperationType::all()"
    );
}
