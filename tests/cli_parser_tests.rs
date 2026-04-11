use clap::Parser;

use rust_jav::cli::{Cli, Command};

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
