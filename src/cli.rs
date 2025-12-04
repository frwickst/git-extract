use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(
    name = "git-extract",
    about = "Split a branch into multiple branches in one go"
)]
pub struct Args {
    /// Base branch to create new branches from
    #[arg(long)]
    pub base: Option<String>,

    /// Keep unassigned commits on current branch (default behavior; provided for explicitness)
    #[arg(long, action = ArgAction::SetTrue)]
    pub default_current: bool,

    /// Drop unassigned commits entirely
    #[arg(long, conflicts_with = "default_current")]
    pub no_current: bool,

    /// Predefine targets as comma or space separated list
    #[arg(long, value_delimiter = ',')]
    pub targets: Vec<String>,

    /// Positional targets (alternate to --targets). Example: git extract feature1 feature2
    #[arg(value_name = "TARGET")]
    pub positional_targets: Vec<String>,

    /// Editor command to use
    #[arg(long)]
    pub editor: Option<String>,

    /// Render/validate only, do not apply changes
    #[arg(long)]
    pub dry_run: bool,

    /// Allow running with a dirty working tree
    #[arg(long)]
    pub allow_dirty: bool,

    /// Use an existing routing file instead of launching the editor (primarily for automation/tests)
    #[arg(long, value_name = "PATH", hide = true)]
    pub routing_file: Option<String>,

    /// Resume a previous extract session after conflicts
    #[arg(long, conflicts_with = "abort")]
    pub r#continue: bool,

    /// Abort a previous extract session
    #[arg(long, conflicts_with = "continue")]
    pub abort: bool,

    /// Do not auto-chdir into conflict worktree on --continue/--abort
    #[arg(long, hide = true)]
    pub no_chdir_conflict: bool,
}

impl Args {
    pub fn keep_current(&self) -> bool {
        // Default is to keep; `--default-current` is explicit, `--no-current` overrides.
        !self.no_current
    }
}
