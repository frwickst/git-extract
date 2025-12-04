# Usage

## Basic invocation
```
git extract [OPTIONS] [TARGET ...]
```
- Positional `TARGET ...` prepopulates target branches (merged with `--targets`).
- `--targets a,b` keeps compatibility and merges with positional.

## Common flags
- `--base <branch>`: base for creating new branches (fallback main/master else HEAD).
- `--default-current` / `--no-current`: keep (default) or drop unassigned commits.
- `--editor <cmd>`: overrides editor (order: --editor > GIT_SEQUENCE_EDITOR > GIT_EDITOR > core.editor > VISUAL > EDITOR > vi).
- `--dry-run`: render/validate only.
- `--allow-dirty`: skip clean check.
- `--routing-file <path>`: use pre-edited routing file (automation/tests).
- `--continue` / `--abort`: resume or cancel after conflicts.
- `--no-chdir-conflict`: opt out of auto-chdir into conflict worktree during continue/abort.

## Conflict workflow
1) On conflict, git-extract keeps the temp worktree, writes `.git/extract-state.json`, and stops.
2) Fix conflicts in that worktree, `git add` your fixes.
3) Run `git extract --continue` (or `--abort`) to proceed; by default it will chdir into that worktree during the command.

## Routing file format
- Header: `target <alias> <branch>`
- Commits (oldest → newest): `<dest> <sha> <subject>` where dest is alias, branch name, or `current`.

## Man page
See `docs/git-extract.1` or install it into your man path (e.g., `/usr/local/share/man/man1/`).
