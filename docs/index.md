# git-extract (Multi-Branch Commit Extractor)

git-extract routes commits from your current branch into multiple target branches in one interactive step. It opens a routing file, you assign each commit to a target (or keep/drop), and it cherry-picks them in order, creating branches if needed.

## Highlights
- Interactive routing file with numeric aliases.
- Automatic branch creation from a base.
- Order-preserving cherry-picks per branch.
- Rebase-style conflict resume with `--continue` / `--abort` and temp worktree isolation.

## Quick start
```bash
cargo install --path . --force
git extract feature1 feature2
# fix conflicts if any, then
git extract --continue
```

## Where to go next
- Usage: [usage](usage.md)
- Architecture: [architecture](architecture.md)
- Contributing: [contributing](contributing.md)
- Man page: [git-extract.1](git-extract.1)
