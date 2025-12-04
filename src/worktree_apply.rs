use crate::git_ops::BaseInfo;
use crate::routing::{ApplySummary, Dest, RoutingPlan};
use crate::state::{BranchQueue, SessionState};
use anyhow::{Context, Result, anyhow};
use git2::{BranchType, Oid, Repository};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug)]
pub enum ApplyOutcome {
    Complete(ApplySummary),
    Conflict(SessionState, String),
}

pub fn apply_plan(repo: &Repository, plan: &RoutingPlan, base: &BaseInfo) -> Result<ApplyOutcome> {
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow!("repository has no working directory"))?;

    let queues = build_branch_queues(plan);
    let wt_path = repo.path().join("extract-wt");
    cleanup_worktree(repo_path, &wt_path)?;
    apply_with_queues(repo, repo_path, base, queues, wt_path, None)
}

pub fn resume_session(
    repo: &Repository,
    state: SessionState,
    allow_dirty: bool,
    no_chdir: bool,
) -> Result<ApplyOutcome> {
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow!("repository has no working directory"))?;
    let wt_path = PathBuf::from(&state.worktree_path);
    if !wt_path.exists() {
        return Err(anyhow!("saved worktree path missing; cannot continue"));
    }
    ensure_worktree_clean(&wt_path, allow_dirty)?;
    let orig_cwd = std::env::current_dir().unwrap_or(repo_path.to_path_buf());
    if !no_chdir {
        let _ = std::env::set_current_dir(&wt_path);
    }
    let res = apply_with_queues(
        repo,
        repo_path,
        &BaseInfo {
            base_oid: Oid::from_str(&state.base_oid)?,
        },
        state.branch_queues.clone(),
        wt_path.clone(),
        Some(state),
    );
    if !no_chdir {
        let _ = std::env::set_current_dir(&orig_cwd);
    }
    res
}

pub fn abort_session(repo: &Repository, state: &SessionState, no_chdir: bool) -> Result<()> {
    let wt_path = PathBuf::from(&state.worktree_path);
    if wt_path.exists() {
        if !no_chdir {
            let _ = std::env::set_current_dir(&wt_path);
        }
        let _ = run_git(
            &wt_path,
            ["-C", wt_path.to_str().unwrap(), "cherry-pick", "--abort"],
        );
        let _ = run_git(
            repo.path(),
            ["worktree", "remove", "--force", wt_path.to_str().unwrap()],
        );
        if !no_chdir {
            let _ = std::env::set_current_dir(repo.path().parent().unwrap_or(Path::new(".")));
        }
    }
    Ok(())
}

fn build_branch_queues(plan: &RoutingPlan) -> Vec<BranchQueue> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for assign in &plan.assignments {
        if let Dest::Branch(b) = &assign.dest {
            if !map.contains_key(b) {
                order.push(b.clone());
            }
            map.entry(b.clone())
                .or_default()
                .push(assign.oid.to_string());
        }
    }
    order
        .into_iter()
        .map(|b| BranchQueue {
            branch: b.clone(),
            commits: map.remove(&b).unwrap_or_default(),
        })
        .collect()
}

fn apply_with_queues(
    repo: &Repository,
    repo_path: &Path,
    base: &BaseInfo,
    mut queues: Vec<BranchQueue>,
    wt_path: PathBuf,
    mut state_opt: Option<SessionState>,
) -> Result<ApplyOutcome> {
    let mut created = Vec::new();
    let mut commits_per_branch: HashMap<String, usize> = HashMap::new();
    let mut current_idx = state_opt
        .as_ref()
        .map(|s| s.current_branch_idx)
        .unwrap_or(0);
    let mut in_conflict = state_opt.as_ref().map(|s| s.in_conflict).unwrap_or(false);

    while current_idx < queues.len() {
        let branch = queues[current_idx].branch.clone();
        // setup worktree for this branch if starting fresh
        if state_opt.is_none() || !in_conflict {
            cleanup_worktree(repo_path, &wt_path)?;
            let (start_spec, branch_existed) = branch_start_spec(repo, &branch, base.base_oid)?;
            if !branch_existed {
                created.push(branch.clone());
            }
            run_git(
                repo_path,
                [
                    "worktree",
                    "add",
                    "--detach",
                    wt_path.to_str().unwrap(),
                    &start_spec,
                ],
            )?;
        }

        // if resuming from conflict, finish current cherry-pick first
        if in_conflict {
            if cherry_pick_in_progress(&wt_path)? {
                run_git(
                    repo_path,
                    ["-C", wt_path.to_str().unwrap(), "cherry-pick", "--continue"],
                )
                .context("continuing cherry-pick")?;
            }
            // whether user already continued or we just did, drop the current commit
            if !queues[current_idx].commits.is_empty() {
                queues[current_idx].commits.remove(0);
            }
            in_conflict = false;
        }

        // apply remaining commits for this branch
        let mut commit_count = commits_per_branch.get(&branch).cloned().unwrap_or(0);
        while let Some(oid_str) = queues[current_idx].commits.first().cloned() {
            let status = run_git_status(wt_path.as_path(), ["cherry-pick", &oid_str]);
            match status {
                Ok(_) => {
                    queues[current_idx].commits.remove(0);
                    commit_count += 1;
                }
                Err(msg) => {
                    let st = SessionState {
                        version: 1,
                        session_id: state_opt
                            .as_ref()
                            .map(|s| s.session_id.clone())
                            .unwrap_or_else(|| Uuid::new_v4().to_string()),
                        worktree_path: wt_path.to_string_lossy().into_owned(),
                        current_branch_idx: current_idx,
                        branch_queues: queues,
                        in_conflict: true,
                        base_oid: state_opt
                            .as_ref()
                            .map(|s| s.base_oid.clone())
                            .unwrap_or_else(|| base.base_oid.to_string()),
                        original_cwd: state_opt
                            .as_ref()
                            .map(|s| s.original_cwd.clone())
                            .unwrap_or_else(|| repo_path.to_string_lossy().into_owned()),
                    };
                    return Ok(ApplyOutcome::Conflict(st, msg));
                }
            }
        }

        let head = run_git(
            repo_path,
            ["-C", wt_path.to_str().unwrap(), "rev-parse", "HEAD"],
        )?;
        let trimmed = head.trim();
        let oid = Oid::from_str(trimmed).context("parse resulting HEAD")?;
        update_branch_ref(repo, &branch, oid)?;
        commits_per_branch.insert(branch.clone(), commit_count);

        cleanup_worktree(repo_path, &wt_path)?;
        current_idx += 1;
        state_opt = None; // after first resume step, treat subsequent branches fresh
    }

    Ok(ApplyOutcome::Complete(ApplySummary {
        created_branches: created,
        commits_per_branch,
    }))
}
fn branch_start_spec(repo: &Repository, branch: &str, base_oid: Oid) -> Result<(String, bool)> {
    if let Ok(existing) = repo.find_branch(branch, BranchType::Local) {
        let _target = existing
            .get()
            .target()
            .ok_or_else(|| anyhow!("branch {branch} has no target"))?;
        return Ok((branch.to_string(), true));
    }
    Ok((base_oid.to_string(), false))
}

fn update_branch_ref(repo: &Repository, branch: &str, target: Oid) -> Result<()> {
    let refname = format!("refs/heads/{branch}");
    repo.reference(&refname, target, true, "git-extract update")?;
    Ok(())
}

fn run_git<S: AsRef<str>>(repo_path: &Path, args: impl IntoIterator<Item = S>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_path);
    for a in args {
        cmd.arg(a.as_ref());
    }
    let output = cmd.output().context("running git command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git command failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_status<S: AsRef<str>>(
    wt_path: &Path,
    args: impl IntoIterator<Item = S>,
) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(wt_path);
    for a in args {
        cmd.arg(a.as_ref());
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.to_string());
    }
    Ok(())
}

fn cleanup_worktree(repo_path: &Path, wt_path: &Path) -> Result<()> {
    if wt_path.exists() {
        let _ = run_git(
            repo_path,
            ["worktree", "remove", "--force", wt_path.to_str().unwrap()],
        );
        if wt_path.exists() {
            std::fs::remove_dir_all(wt_path).ok();
        }
    }
    Ok(())
}

fn ensure_worktree_clean(wt_path: &Path, allow_dirty: bool) -> Result<()> {
    if allow_dirty {
        return Ok(());
    }
    let status = run_git(
        wt_path,
        ["-C", wt_path.to_str().unwrap(), "status", "--porcelain"],
    )?;
    if !status.trim().is_empty() {
        // allow staged/non-conflict? To keep simple, require clean
        return Err(anyhow!(
            "worktree is not clean; resolve conflicts and stage changes before --continue"
        ));
    }
    Ok(())
}

fn cherry_pick_in_progress(wt_path: &Path) -> Result<bool> {
    let path = run_git(
        wt_path,
        [
            "-C",
            wt_path.to_str().unwrap(),
            "rev-parse",
            "--git-path",
            "CHERRY_PICK_HEAD",
        ],
    )?;
    let p = PathBuf::from(path.trim());
    Ok(p.exists())
}
