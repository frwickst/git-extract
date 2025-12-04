use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git run");
    assert!(status.success(), "git {:?} failed", args);
}

fn git_out(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git run");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn setup_conflict_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(&repo, &["init"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);

    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    git(&repo, &["commit", "-m", "base"]);

    // main change diverging
    fs::write(repo.join("file.txt"), "main-change\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    git(&repo, &["commit", "-m", "main change"]);

    // work branch from base (before main change)
    let base_commit = git_out(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&repo, &["checkout", "-b", "work", &base_commit]);

    fs::write(repo.join("file.txt"), "work1\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    git(&repo, &["commit", "-m", "work1"]);
    fs::write(repo.join("file.txt"), "work2\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    git(&repo, &["commit", "-m", "work2"]);

    tmp
}

#[test]
fn conflict_then_continue() {
    let tmp = setup_conflict_repo();
    let repo = tmp.path();

    // routing file assigns both commits to feature
    let c1 = git_out(&repo, &["rev-parse", "--short", "HEAD~1"]);
    let c2 = git_out(&repo, &["rev-parse", "--short", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(
        &routing,
        format!("target 1 feature\n1 {c1} work1\n1 {c2} work2\n"),
    )
    .unwrap();

    let bin = assert_cmd::cargo::cargo_bin!("git-extract");
    let assert = Command::new(&bin)
        .current_dir(&repo)
        .args([
            "--routing-file",
            routing.to_str().unwrap(),
            "feature",
            "--allow-dirty",
        ])
        .assert();
    // conflict expected but process exits 0 (prints instruction)
    assert.success();

    let state_path = repo.join(".git").join("extract-state.json");
    assert!(state_path.exists());
    let state: Value = serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    let wt_path = PathBuf::from(state["worktree_path"].as_str().unwrap());
    assert!(wt_path.exists());

    // resolve conflict in worktree: set desired content
    fs::write(wt_path.join("file.txt"), "work1\n").unwrap();
    git(&wt_path, &["add", "file.txt"]);
    git(&wt_path, &["cherry-pick", "--continue"]);

    Command::new(&bin)
        .current_dir(&repo)
        .args(["--continue", "--allow-dirty"])
        .assert()
        .success();

    // state removed and branch updated
    assert!(!state_path.exists());
    let log = git_out(&repo, &["log", "--oneline", "feature"]);
    assert!(log.contains("work1"));
    assert!(log.contains("work2"));
}

#[test]
fn conflict_abort() {
    let tmp = setup_conflict_repo();
    let repo = tmp.path();

    let c1 = git_out(&repo, &["rev-parse", "--short", "HEAD~1"]);
    let c2 = git_out(&repo, &["rev-parse", "--short", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(
        &routing,
        format!("target 1 feature\n1 {c1} work1\n1 {c2} work2\n"),
    )
    .unwrap();

    let bin = assert_cmd::cargo::cargo_bin!("git-extract");
    Command::new(&bin)
        .current_dir(&repo)
        .args([
            "--routing-file",
            routing.to_str().unwrap(),
            "feature",
            "--allow-dirty",
        ])
        .assert()
        .success();

    let state_path = repo.join(".git").join("extract-state.json");
    assert!(state_path.exists());
    let wt_path = {
        let v: Value = serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
        PathBuf::from(v["worktree_path"].as_str().unwrap())
    };
    assert!(wt_path.exists());

    Command::new(&bin)
        .current_dir(&repo)
        .args(["--abort", "--allow-dirty"])
        .assert()
        .success();

    assert!(!state_path.exists());
    assert!(!wt_path.exists());
    // feature branch should not exist
    let status = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["show-ref", "refs/heads/feature"])
        .status()
        .unwrap();
    assert!(!status.success());
}
