# Publishing to crates.io

This workspace ships three crates in dependency order:

```
rustdata-macros  →  rustdata-migrations  →  rustdata-core
```

`migration-test` is an integration-test crate and must **not** be published.

---

## Prerequisites

- A [crates.io](https://crates.io) account.
- A valid [`CARGO_REGISTRY_TOKEN`](https://crates.io/settings/tokens) in `~/.cargo/credentials.toml` or exported as an environment variable.
- Each published crate's `Cargo.toml` `[package]` section must contain at minimum `name`, `version`, `edition`, `license`, `description`, `homepage`, `repository`, `keywords`, and `categories`.
- `README.md` must contain useful crate-level docs (crates.io renders it).

---

## `scripts/release.sh` (full automated release)

`scripts/release.sh` is the single source of truth for the release workflow.
It calls the Makefile targets in the order they need to run.

```bash
# Dry-run — quality gate + dry-run publish, no files modified
./scripts/release.sh --dry-run

# Interactive — prompts for major/minor/patch
./scripts/release.sh

# Non-interactive minor bump, no confirmation prompt
./scripts/release.sh --bump minor --yes

# Non-interactive patch bump
./scripts/release.sh --bump patch --yes
```

What it does step by step:

| Step | Command | Description |
|---|---|---|
| 1 | `make check` | `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test --workspace --all-features` |
| 2 | `make publish-dry-run` | `cargo publish --dry-run` for all 3 crates in dependency order |
| 3 | `make bump-version VER=…` | `cargo set-version --workspace` to synchronise all 3 crates |
| 4 | `make changelog` | Appends `## [x.y.z] - YYYY-MM-DD` to `CHANGELOG.md` if not already present |
| 5 | `make tag` | `git commit` + GPG-signed `git tag` + `git push --follow-tags` to `origin` and `rustdata` remotes |
| 6 | `make publish-upload` | `cargo publish` for all 3 crates in dependency order |

Requires `python3`, `git` (with GPG/SSH tag signing), and `cargo-set-version` (`cargo install cargo-set-version`).

---

## Makefile targets (individual steps)

Run any step individually if you want more control:

```bash
# 1. Format check + lint + tests
make check

# 2. Preview .crate files without uploading
make publish-dry-run

# 3. Bump workspace version (uses cargo-set-version)
make bump-version VER=0.2.0

# 4. Add a new [x.y.z] section to CHANGELOG.md
make changelog

# 5. Git commit + signed tag + push
make tag

# 6. Upload to crates.io (all 3 crates in order)
make publish-upload
```

### `make publish`

Runs the full pipeline in one Make command:

```bash
make publish   # runs: check → changelog → publish-dry-run → tag → publish-upload
```

Note that `make publish` does *not* include `make publish-dry-run`; add it as a pre-check manually if desired:

```bash
make check && make publish-dry-run && make publish
```

---

## Verify on crates.io

```bash
cargo search rustdata-core
cargo search rustdata-macros
cargo search rustdata-migrations
```

---

## Post-release checklist

- [ ] Confirm all 3 crates appear on crates.io with the correct version.
- [ ] Verify the GitHub release tag created by `make tag` is live.
- [ ] Announce in project channels / discussions.
