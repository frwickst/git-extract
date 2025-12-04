use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STATE_FILE: &str = "extract-state.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchQueue {
    pub branch: String,
    pub commits: Vec<String>, // remaining commits (full shas), oldest -> newest
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionState {
    pub version: u32,
    pub session_id: String,
    pub worktree_path: String,
    pub current_branch_idx: usize,
    pub branch_queues: Vec<BranchQueue>,
    pub in_conflict: bool,
    pub base_oid: String,
    pub original_cwd: String,
}

impl SessionState {
    pub fn new(
        worktree_path: &Path,
        branch_queues: Vec<BranchQueue>,
        base_oid: git2::Oid,
        original_cwd: &Path,
    ) -> Self {
        Self {
            version: 1,
            session_id: Uuid::new_v4().to_string(),
            worktree_path: worktree_path.to_string_lossy().into_owned(),
            current_branch_idx: 0,
            branch_queues,
            in_conflict: false,
            base_oid: base_oid.to_string(),
            original_cwd: original_cwd.to_string_lossy().into_owned(),
        }
    }
}

pub fn state_path(repo: &git2::Repository) -> PathBuf {
    repo.path().join(STATE_FILE)
}

pub fn load_state(repo: &git2::Repository) -> Result<SessionState> {
    let path = state_path(repo);
    let content =
        fs::read_to_string(&path).with_context(|| format!("reading state file {path:?}"))?;
    let state: SessionState = serde_json::from_str(&content).context("parsing state file")?;
    Ok(state)
}

pub fn save_state(repo: &git2::Repository, state: &SessionState) -> Result<()> {
    let path = state_path(repo);
    let data = serde_json::to_string_pretty(state)?;
    fs::write(&path, data).with_context(|| format!("writing state file {path:?}"))?;
    Ok(())
}

pub fn remove_state(repo: &git2::Repository) -> Result<()> {
    let path = state_path(repo);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("removing state file {path:?}"))?;
    }
    Ok(())
}

pub fn ensure_no_state(repo: &git2::Repository) -> Result<()> {
    let path = state_path(repo);
    if path.exists() {
        return Err(anyhow!(
            "an extract session is already in progress; run --continue or --abort"
        ));
    }
    Ok(())
}
