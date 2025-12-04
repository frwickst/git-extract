use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use cli::Args;
use git_extract::{cli, git_ops, routing, state, worktree_apply};

fn main() -> Result<()> {
    let args = Args::parse();
    let repo = git2::Repository::discover(".").context("not inside a git repository")?;

    // Allow `git extract help` to show clap help
    if std::env::args().nth(1).as_deref() == Some("help") {
        Args::command().print_help()?;
        println!();
        return Ok(());
    }

    // resume/abort existing session
    if args.r#continue || args.abort {
        let st = state::load_state(&repo).context("no extract session to continue/abort")?;
        if args.abort {
            worktree_apply::abort_session(&repo, &st, args.no_chdir_conflict)?;
            state::remove_state(&repo)?;
            println!("extract session aborted; temp worktree removed");
            return Ok(());
        }
        let outcome =
            worktree_apply::resume_session(&repo, st, args.allow_dirty, args.no_chdir_conflict)?;
        handle_outcome(&repo, outcome)?;
        return Ok(());
    }

    state::ensure_no_state(&repo)?;
    git_ops::ensure_clean(&repo, args.allow_dirty)?;

    let base_info = git_ops::detect_base(&repo, args.base.clone())?;
    let commits = git_ops::collect_commits(&repo, base_info.base_oid)?;

    let target_defs = routing::TargetDefs::from_args(&args);
    let draft_path = routing::render_routing_file(&target_defs, &commits)?;

    let routing_path = if let Some(path) = &args.routing_file {
        std::path::PathBuf::from(path)
    } else {
        routing::launch_editor(&args, &repo, &draft_path)?;
        draft_path.clone()
    };

    let plan =
        routing::parse_routing_file(&routing_path, &commits, &target_defs, args.keep_current())?;

    if args.dry_run {
        routing::print_plan_summary(&plan);
        return Ok(());
    }

    let outcome = worktree_apply::apply_plan(&repo, &plan, &base_info)?;
    handle_outcome(&repo, outcome)?;

    Ok(())
}

fn handle_outcome(repo: &git2::Repository, outcome: worktree_apply::ApplyOutcome) -> Result<()> {
    match outcome {
        worktree_apply::ApplyOutcome::Complete(summary) => {
            state::remove_state(repo)?;
            routing::print_apply_summary(&summary);
        }
        worktree_apply::ApplyOutcome::Conflict(st, msg) => {
            state::save_state(repo, &st)?;
            println!("Conflict encountered: {msg}");
            println!("Resolve conflicts in worktree: {}", st.worktree_path);
            println!("Then run: git extract --continue  (or --abort to cancel)");
        }
    }
    Ok(())
}
