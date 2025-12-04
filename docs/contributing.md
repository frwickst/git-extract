# Contributing

## Dev setup
- Install Rust via rustup.
- Components: `rustfmt`, `clippy`.
- Optional: `cross` and Docker for cross/musl builds.

## Common tasks
- Format: `cargo fmt`
- Lint: `cargo clippy -- -D warnings`
- Tests: `cargo test`
- Release (host): `cargo build --release`
- Static-ish Linux: `LIBGIT2_SYS_USE_PKG_CONFIG=0 OPENSSL_STATIC=1 RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-unknown-linux-musl`

## Conflict resume
- On conflict, state is saved to `.git/extract-state.json` and the temp worktree is kept.
- Fix conflicts in that worktree, stage, then `git extract --continue` (or `--abort`).
- Auto-chdir during continue/abort unless `--no-chdir-conflict`.

## Docs
- MkDocs + Material. Local preview: `pip install mkdocs-material` then `mkdocs serve`.

## CI
- GitHub Actions runs fmt, clippy, tests, builds macOS (arm64) and Linux x86_64 musl binaries, and builds docs.
