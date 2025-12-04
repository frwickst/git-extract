use crate::cli::Args;
use crate::git_ops::CommitInfo;
use anyhow::{Context, Result, anyhow};
use git2::Oid;
use git2::Repository;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TargetDef {
    pub alias: u32,
    pub branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct TargetDefs {
    pub targets: Vec<TargetDef>,
}

#[derive(Debug, Clone)]
pub enum Dest {
    Branch(String),
    Current,
    Drop,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub oid: Oid,
    pub dest: Dest,
}

#[derive(Debug, Clone)]
pub struct RoutingPlan {
    pub assignments: Vec<Assignment>,
}

impl TargetDefs {
    pub fn from_args(args: &Args) -> Self {
        let mut combined: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for name in args.targets.iter().chain(args.positional_targets.iter()) {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                combined.push(trimmed.to_string());
            }
        }

        let targets = combined
            .iter()
            .enumerate()
            .map(|(idx, name)| TargetDef {
                alias: (idx + 1) as u32,
                branch: name.to_string(),
            })
            .collect();

        TargetDefs { targets }
    }
}

pub fn render_routing_file(targets: &TargetDefs, commits: &[CommitInfo]) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    let fname = format!("git-extract-{}.txt", std::process::id());
    path.push(fname);
    let mut file = fs::File::create(&path).context("create routing file")?;

    for t in &targets.targets {
        writeln!(file, "target {} {}", t.alias, t.branch)?;
    }
    if !targets.targets.is_empty() {
        writeln!(file)?;
    }
    for commit in commits {
        writeln!(file, "current {} {}", commit.short, commit.summary)?;
    }
    Ok(path)
}

pub fn launch_editor(args: &Args, repo: &Repository, path: &Path) -> Result<()> {
    let editor = resolve_editor(args, repo);

    let status = Command::new(editor)
        .arg(path)
        .status()
        .context("launching editor")?;
    if !status.success() {
        return Err(anyhow!("editor exited with error"));
    }
    Ok(())
}

pub fn parse_routing_file(
    path: &Path,
    commits: &[CommitInfo],
    targets: &TargetDefs,
    keep_current: bool,
) -> Result<RoutingPlan> {
    let content = fs::read_to_string(path).context("read routing file")?;

    let mut alias_map: HashMap<u32, String> = HashMap::new();
    for t in &targets.targets {
        alias_map.insert(t.alias, t.branch.clone());
    }

    let mut assignments: Vec<Assignment> = Vec::new();
    let mut seen_oids: HashSet<git2::Oid> = HashSet::new();

    let commit_map: HashMap<String, Oid> =
        commits.iter().map(|c| (c.short.clone(), c.oid)).collect();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("target ") {
            let mut parts = trimmed.split_whitespace();
            let _ = parts.next(); // target
            let alias = parts
                .next()
                .ok_or_else(|| anyhow!("target line missing alias"))?
                .parse::<u32>()
                .context("invalid target alias")?;
            let branch = parts
                .next()
                .ok_or_else(|| anyhow!("target line missing branch"))?;
            alias_map.insert(alias, branch.to_string());
            continue;
        }

        let mut parts = trimmed.splitn(3, ' ');
        let dest_token = parts
            .next()
            .ok_or_else(|| anyhow!("missing destination token"))?;
        let sha_token = parts
            .next()
            .ok_or_else(|| anyhow!("missing commit sha token"))?;
        let subject = parts.next().unwrap_or("");
        if subject.is_empty() {
            return Err(anyhow!("line must include a subject: {trimmed}"));
        }

        let oid = commit_map
            .get(sha_token)
            .copied()
            .or_else(|| resolve_full_sha(commits, sha_token))
            .ok_or_else(|| anyhow!("unknown commit sha {sha_token}"))?;
        if !seen_oids.insert(oid) {
            return Err(anyhow!("duplicate assignment for commit {sha_token}"));
        }

        let dest = parse_dest(dest_token, &alias_map, keep_current)?;
        assignments.push(Assignment { oid, dest });
    }

    // Ensure every commit is assigned
    if assignments.len() != commits.len() {
        return Err(anyhow!("every listed commit must be assigned"));
    }

    Ok(RoutingPlan { assignments })
}

fn parse_dest(token: &str, alias_map: &HashMap<u32, String>, keep_current: bool) -> Result<Dest> {
    if token == "current" {
        return if keep_current {
            Ok(Dest::Current)
        } else {
            Ok(Dest::Drop)
        };
    }
    if let Ok(num) = token.parse::<u32>() {
        if let Some(branch) = alias_map.get(&num) {
            return Ok(Dest::Branch(branch.clone()));
        }
        return Err(anyhow!("unknown target alias {num}"));
    }
    // branch name
    Ok(Dest::Branch(token.to_string()))
}

fn resolve_full_sha(commits: &[CommitInfo], token: &str) -> Option<Oid> {
    if token.len() < 7 {
        return None;
    }
    commits
        .iter()
        .find(|c| c.oid.to_string().starts_with(token))
        .map(|c| c.oid)
}

pub fn print_plan_summary(plan: &RoutingPlan) {
    println!("Dry run: assignments");
    let mut per_branch: HashMap<String, usize> = HashMap::new();
    let mut drops = 0usize;
    for a in &plan.assignments {
        match &a.dest {
            Dest::Branch(b) => *per_branch.entry(b.clone()).or_insert(0) += 1,
            Dest::Current => *per_branch.entry("current".into()).or_insert(0) += 1,
            Dest::Drop => drops += 1,
        }
    }
    for (branch, count) in per_branch {
        println!("  {branch}: {count} commits");
    }
    if drops > 0 {
        println!("  dropped: {drops} commits");
    }
}

#[derive(Debug, Clone)]
pub struct ApplySummary {
    pub created_branches: Vec<String>,
    pub commits_per_branch: HashMap<String, usize>,
}

pub fn print_apply_summary(summary: &ApplySummary) {
    println!("Apply complete");
    if !summary.created_branches.is_empty() {
        println!("  created: {}", summary.created_branches.join(", "));
    }
    for (branch, count) in &summary.commits_per_branch {
        println!("  {branch}: {count} commits");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_ops::CommitInfo;
    use git2::Oid;

    fn mk_commit(idx: u8) -> CommitInfo {
        let hex = format!("{:07x}{:033x}", idx as u32, idx);
        let oid = Oid::from_str(&hex).unwrap();
        CommitInfo {
            oid,
            short: hex[..7].to_string(),
            summary: format!("Commit {idx}"),
        }
    }

    #[test]
    fn parse_basic_plan() {
        let commits = vec![mk_commit(1), mk_commit(2)];
        let targets = TargetDefs {
            targets: vec![TargetDef {
                alias: 1,
                branch: "feature".into(),
            }],
        };

        let path = std::env::temp_dir().join("routing-test.txt");
        let content = format!(
            "target 1 feature\n1 {} Commit 1\ncurrent {} Commit 2\n",
            commits[0].short, commits[1].short
        );
        std::fs::write(&path, content).unwrap();

        let plan = parse_routing_file(&path, &commits, &targets, true).unwrap();
        assert_eq!(plan.assignments.len(), 2);
        assert!(matches!(plan.assignments[0].dest, Dest::Branch(ref b) if b == "feature"));
        assert!(matches!(plan.assignments[1].dest, Dest::Current));
    }

    #[test]
    fn parse_rejects_unknown_commit() {
        let commits = vec![mk_commit(1)];
        let targets = TargetDefs { targets: vec![] };
        let path = std::env::temp_dir().join("routing-test-err.txt");
        std::fs::write(&path, "current deadbeef Missing\n").unwrap();
        let err = parse_routing_file(&path, &commits, &targets, true).unwrap_err();
        assert!(err.to_string().contains("unknown commit"));
    }
}
fn resolve_editor(args: &Args, repo: &Repository) -> String {
    if let Some(e) = &args.editor {
        return e.clone();
    }
    for var in ["GIT_SEQUENCE_EDITOR", "GIT_EDITOR"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return v;
            }
        }
    }
    if let Ok(cfg) = repo.config() {
        if let Ok(v) = cfg.get_string("core.editor") {
            if !v.trim().is_empty() {
                return v;
            }
        }
    }
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return v;
            }
        }
    }
    "vi".to_string()
}
