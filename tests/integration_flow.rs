use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn git(repo: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git {:?} failed", args);
}

fn git_out(repo: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(out.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn full_flow_routes_commits() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();

    git(repo, &["init"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Tester"]);

    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "base"]);

    git(repo, &["checkout", "-b", "work"]);

    fs::write(repo.join("file.txt"), "base\nchange1\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "change1"]);
    let c1 = git_out(repo, &["rev-parse", "--short", "HEAD"]);

    fs::write(repo.join("file.txt"), "base\nchange1\nchange2\n").unwrap();
    git(repo, &["add", "file.txt"]);
    git(repo, &["commit", "-m", "change2"]);
    let c2 = git_out(repo, &["rev-parse", "--short", "HEAD"]);

    let routing = repo.join("routing.txt");
    let content = format!("target 1 feature1\n1 {c1} change1\n1 {c2} change2\n");
    fs::write(&routing, content).unwrap();

    let bin = assert_cmd::cargo::cargo_bin!("git-extract");

    Command::new(bin)
        .current_dir(repo)
        .args([
            "feature1",
            "--routing-file",
            routing.to_str().unwrap(),
            "--allow-dirty",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feature1"));

    let log = git_out(repo, &["log", "--oneline", "feature1"]);
    assert!(log.contains("change1"));
    assert!(log.contains("change2"));
}
