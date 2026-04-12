use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::tui::state::OperationType;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Launch the TUI interface
    Tui(TuiArgs),
    /// Preview or apply file operations
    Ops(OpsArgs),
    /// Preview or apply actor directory hard links derived from NFO metadata
    ActorLinks(ActorLinksArgs),
}

#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Source directory to inspect with the TUI
    #[arg(short = 'd', long)]
    pub dir: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct OpsArgs {
    /// Source directory containing JAV files
    #[arg(short = 'd', long)]
    pub dir: PathBuf,

    /// Apply filesystem mutations; otherwise preview is the default
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON output
    #[arg(long)]
    pub json: bool,

    /// Limit execution to one or more specific operations
    #[arg(long = "op", value_enum)]
    pub ops: Vec<CliOperation>,
}

impl OpsArgs {
    pub fn selected_operations(&self) -> Vec<OperationType> {
        if self.ops.is_empty() {
            OperationType::all()
        } else {
            self.ops.iter().copied().map(Into::into).collect()
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct ActorLinksArgs {
    /// Source directory containing media files and NFO metadata
    #[arg(long)]
    pub source: PathBuf,

    /// Target root where actor directories should be created
    #[arg(long)]
    pub actors_root: PathBuf,

    /// Directories to exclude from scanning (can be repeated)
    /// If actors_root is inside source, it is auto-excluded
    #[arg(long)]
    pub exclude: Vec<PathBuf>,

    /// Apply filesystem mutations; otherwise preview is the default
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON output
    #[arg(long)]
    pub json: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliOperation {
    DeleteAdFiles,
    OrganizeByCode,
    CleanEmptyDirs,
    StandardizeNames,
    ExtractCodes,
    CategorizeFiles,
    MoveOrigin,
    RemoveDuplicates,
}

impl From<CliOperation> for OperationType {
    fn from(value: CliOperation) -> Self {
        match value {
            CliOperation::DeleteAdFiles => OperationType::DeleteAdFiles,
            CliOperation::OrganizeByCode => OperationType::OrganizeByCode,
            CliOperation::CleanEmptyDirs => OperationType::CleanEmptyDirs,
            CliOperation::StandardizeNames => OperationType::StandardizeNames,
            CliOperation::ExtractCodes => OperationType::ExtractCodes,
            CliOperation::CategorizeFiles => OperationType::CategorizeFiles,
            CliOperation::MoveOrigin => OperationType::MoveOrigin,
            CliOperation::RemoveDuplicates => OperationType::RemoveDuplicates,
        }
    }
}
