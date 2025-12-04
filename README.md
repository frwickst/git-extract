# git-extract (Multi-Branch Commit Extractor)

Prototype implementation of the interactive branching tool described in `project.md`.

## Usage

Install the binary named `git-extract` on your `PATH` to use it as `git extract ...`.

```
git extract [OPTIONS] [TARGET ...]
```

- `--base`: base branch/commit for creating new branches (defaults to main/master fallback).
- positional `TARGET ...`: prepopulate targets (e.g., `git extract feature1 feature2`).
- `--targets`: prepopulate target lines (comma-separated) — kept for compatibility and merged with positional inputs.
- `--default-current`: explicit no-op; unassigned commits stay on current (default behavior).
- `--no-current`: unassigned commits are dropped entirely.
- `--editor`: override editor (fallback GIT_EDITOR > VISUAL > EDITOR > vi).
  - Git config `core.editor` is also honored (precedence: CLI `--editor` > `GIT_SEQUENCE_EDITOR` > `GIT_EDITOR` > `core.editor` > `VISUAL` > `EDITOR` > `vi`).
- `--dry-run`: render/validate only; no branch updates.
- `--allow-dirty`: skip clean-worktree check.
- `--routing-file <path>` (hidden/automation): use a pre-edited routing file instead of launching an editor.
- `--continue`: resume after resolving conflicts in the temp worktree.
- `--abort`: abort an in-progress extract session and clean up.
- `--no-chdir-conflict` (hidden): skip auto-chdir into the conflict worktree during --continue/--abort.

Notes:
- Requires being on a branch (detached HEAD is rejected).
- If neither `main` nor `master` exist and `--base` is not provided, the tool falls back to `HEAD` and prints a note.

More detail: see `docs/architecture.md` for an under-the-hood walkthrough and flow diagram.

## Building and installing

### Development workflow
```bash
cargo build        # debug build
cargo test         # run tests
target/debug/git-extract --help
```
For convenient use as `git extract` during development, either add `target/debug` to your `PATH` or run via `cargo run -- <args>`.

### Release build and install
```bash
cargo build --release
```
Then copy or symlink the binary onto your PATH, e.g.:
```bash
sudo cp target/release/git-extract /usr/local/bin/
# or user-local:
cp target/release/git-extract ~/.local/bin/
```

Alternatively, install directly with Cargo:
```bash
cargo install --path . --force
```
This places `git-extract` in `~/.cargo/bin`; add `export PATH="$HOME/.cargo/bin:$PATH"` to your shell profile if needed.

Once `git-extract` is on your PATH, Git will recognize it as `git extract` automatically.

Optional man page:
- Install `docs/git-extract.1` to your man path, e.g. `sudo cp docs/git-extract.1 /usr/local/share/man/man1/`
- Then `git help extract` will show the man page.

Docs site: built with MkDocs + Material (`mkdocs.yml`, content in `docs/`).
CI: see `.github/workflows/ci.yml` (fmt, clippy, tests, macOS arm64 + Linux musl builds, docs build, artifacts).

Workflow:
1. Tool lists commits on current branch since merge-base with base.
2. Opens a routing file:
   - `target <alias> <branch>` header lines (predefined if `--targets`).
   - Commit lines: `current <sha> <subject>` (oldest → newest).
3. You edit prefixes to route commits:
   - `current` to keep, alias number or branch name to send elsewhere.
4. On save, the file is validated; on apply, branches are created if missing and commits cherry-picked via temporary worktrees to avoid touching your working tree.

Conflict handling: on the first cherry-pick conflict for a branch, the tool leaves the temp
worktree intact, writes `.git/extract-state.json`, and stops with instructions. Resolve conflicts
in that worktree, `git add` your fixes, then run `git extract --continue` to resume where it
stopped (or `git extract --abort` to cancel). Branches are only updated after their commits apply
cleanly.

Tests:
- Parser/validation unit tests live in `src/routing.rs` (run `cargo test`).
