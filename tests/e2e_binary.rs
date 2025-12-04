use assert_cmd::prelude::*;
use predicates::str::contains;
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

fn init_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(repo, &["init"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Tester"]);
    tmp
}

#[test]
fn e2e_happy_path_routing() {
    let tmp = init_repo();
    let repo = tmp.path();

    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "base"]);

    fs::write(repo.join("file.txt"), "c1\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "c1"]);
    fs::write(repo.join("file.txt"), "c2\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "c2"]);

    let c1 = git_out(repo, &["rev-parse", "--short", "HEAD~1"]);
    let c2 = git_out(repo, &["rev-parse", "--short", "HEAD"]);
    let base_sha = git_out(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(
        &routing,
        format!("target 1 feature\n1 {c1} c1\ncurrent {c2} c2\n"),
    )
    .unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("git-extract"))
        .current_dir(repo)
        .args([
            "--routing-file",
            routing.to_str().unwrap(),
            "--allow-dirty",
            "--base",
            base_sha.as_str(),
        ])
        .assert()
        .success();

    let feature_log = git_out(repo, &["log", "--oneline", "feature"]);
    assert!(feature_log.contains("c1"));
    let main_log = git_out(repo, &["log", "--oneline", "HEAD"]);
    assert!(main_log.contains("c2"));
    assert!(!repo.join(".git").join("extract-state.json").exists());
}

#[test]
fn e2e_dry_run_creates_nothing() {
    let tmp = init_repo();
    let repo = tmp.path();

    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "base"]);
    fs::write(repo.join("file.txt"), "c1\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "c1"]);

    let sha = git_out(repo, &["rev-parse", "--short", "HEAD"]);
    let base_sha = git_out(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(&routing, format!("target 1 feature\n1 {sha} c1\n")).unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("git-extract"))
        .current_dir(repo)
        .args([
            "--routing-file",
            routing.to_str().unwrap(),
            "--dry-run",
            "--allow-dirty",
            "--base",
            base_sha.as_str(),
        ])
        .assert()
        .success();

    // feature branch should not exist
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "refs/heads/feature"])
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!repo.join(".git").join("extract-state.json").exists());
}

#[test]
fn e2e_conflict_continue() {
    let tmp = init_repo();
    let repo = tmp.path();

    // base
    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "base"]);

    // main change
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "mainchange"]);

    // work branch from base
    let base = git_out(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(repo, &["checkout", base.as_str()]);
    git(repo, &["checkout", "-b", "work"]);
    fs::write(repo.join("file.txt"), "work1\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "work1"]);

    // route work1 to feature (will conflict with main change)
    let c1 = git_out(repo, &["rev-parse", "--short", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(&routing, format!("target 1 feature\n1 {c1} work1\n")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin!("git-extract");
    Command::new(&bin)
        .current_dir(repo)
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
    let wt_path: PathBuf = {
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
        PathBuf::from(v["worktree_path"].as_str().unwrap())
    };
    assert!(wt_path.exists());

    // resolve conflict: keep "main" + newline
    fs::write(wt_path.join("file.txt"), "main\nwork1\n").unwrap();
    git(&wt_path, &["add", "file.txt"]);
    git(&wt_path, &["cherry-pick", "--continue"]);

    Command::new(&bin)
        .current_dir(repo)
        .args(["--continue", "--allow-dirty"])
        .assert()
        .success();

    assert!(!state_path.exists());
    let flog = git_out(repo, &["log", "--oneline", "feature"]);
    assert!(flog.contains("work1"));
}

#[test]
fn e2e_conflict_abort() {
    let tmp = init_repo();
    let repo = tmp.path();

    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "base"]);
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "mainchange"]);

    let base = git_out(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(repo, &["checkout", base.as_str()]);
    git(repo, &["checkout", "-b", "work"]);
    fs::write(repo.join("file.txt"), "work1\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "work1"]);

    let c1 = git_out(repo, &["rev-parse", "--short", "HEAD"]);
    let routing = repo.join("routing.txt");
    fs::write(&routing, format!("target 1 feature\n1 {c1} work1\n")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin!("git-extract");
    Command::new(&bin)
        .current_dir(repo)
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
    Command::new(&bin)
        .current_dir(repo)
        .args(["--abort", "--allow-dirty"])
        .assert()
        .success();

    assert!(!state_path.exists());
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "refs/heads/feature"])
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn e2e_help_outputs() {
    let bin = assert_cmd::cargo::cargo_bin!("git-extract");
    Command::new(&bin)
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("git extract"));

    Command::new(&bin)
        .arg("help")
        .assert()
        .success()
        .stdout(contains("git extract"));
}
