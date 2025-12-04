use anyhow::{Context, Result, anyhow};
use git2::{Oid, Repository, Sort};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BaseInfo {
    pub base_oid: Oid,
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub short: String,
    pub summary: String,
}

pub fn ensure_clean(repo: &Repository, allow_dirty: bool) -> Result<()> {
    if repo.head_detached()? {
        return Err(anyhow!(
            "detached HEAD is not supported; please check out a branch"
        ));
    }
    if allow_dirty {
        return Ok(());
    }
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo.workdir().unwrap_or_else(|| std::path::Path::new(".")))
        .output()
        .context("checking worktree cleanliness")?;
    if !output.status.success() {
        return Err(anyhow!("git status failed"));
    }
    if !output.stdout.is_empty() {
        return Err(anyhow!(
            "working tree is dirty; commit/stash or pass --allow-dirty"
        ));
    }
    Ok(())
}

pub fn detect_base(repo: &Repository, user_base: Option<String>) -> Result<BaseInfo> {
    if let Some(base) = user_base {
        let oid = resolve_ref(repo, &base)?;
        return Ok(BaseInfo { base_oid: oid });
    }

    for candidate in ["origin/main", "main", "origin/master", "master"] {
        if let Ok(oid) = resolve_ref(repo, candidate) {
            return Ok(BaseInfo { base_oid: oid });
        }
    }

    let head = repo.head()?.target().context("HEAD has no target")?;
    eprintln!(
        "note: no main/master found; defaulting base to HEAD ({})",
        head
    );
    Ok(BaseInfo { base_oid: head })
}

fn resolve_ref(repo: &Repository, name: &str) -> Result<Oid> {
    if let Ok(reference) = repo.revparse_single(name) {
        return Ok(reference.id());
    }
    Err(anyhow!("unable to resolve ref {name}"))
}

pub fn collect_commits(repo: &Repository, base_oid: Oid) -> Result<Vec<CommitInfo>> {
    let head = repo.head()?.target().context("HEAD has no target")?;
    let merge_base = repo.merge_base(head, base_oid).unwrap_or(base_oid);

    let mut revwalk = repo.revwalk()?;
    revwalk.push(head)?;
    revwalk.hide(merge_base)?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    let mut commits: Vec<CommitInfo> = Vec::new();
    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let summary = commit
            .summary_bytes()
            .map(|s| String::from_utf8_lossy(s).to_string())
            .unwrap_or_else(|| "(no summary)".to_string());
        let short = oid.to_string()[..7].to_string();
        commits.push(CommitInfo {
            oid,
            short,
            summary,
        });
    }
    commits.reverse();
    Ok(commits)
}
