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

## Using `scripts/release.sh` (recommended)

The project ships a full automated release script. This is the single source of truth for the release workflow.

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

The script performs all steps in order:

| Step | What it does |
|---|---|
| `make check` | `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test --workspace --all-features` |
| `make publish-dry-run` | `cargo publish --dry-run` for all 3 crates in order |
| `make bump-version VER=…` | Uses `cargo set-version --workspace` to synchronise all 3 crates |
| `make changelog` | Appends a new `## [x.y.z] - YYYY-MM-DD` section to `CHANGELOG.md` if one does not already exist |
| `make tag` | `git commit` + GPG-signed `git tag` + `git push --follow-tags` to `origin` and `rustdata` remotes |
| `make publish-upload` | `cargo publish` for all 3 crates in order |

Requires `python3`, `git` (with GPG/SSH tag signing), and `cargo-set-version` (`cargo install cargo-set-version`).

---

## Makefile targets (manual workflow)

If you prefer to run steps individually instead of using the release script:

```bash
# 1. Quality gate
make check

# 2. Dry-run publish (previews .crate files, does not upload)
make publish-dry-run

# 3. Bump version across all crates
make bump-version VER=0.2.0

# 4. Update CHANGELOG.md
make changelog

# 5. Commit + GPG-signed tag + push
make tag

# 6. Publish to crates.io (all 3 crates in order)
make publish-upload
```

### `make publish`

Runs the full pipeline in one command:

```bash
make publish   # equivalent to: check → changelog → publish-dry-run → tag → publish-upload
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

- [ ] Verify all 3 crates appear on crates.io with the correct version.
- [ ] Confirm the GitHub release (created by `make tag`) is live and linked.
- [ ] Announce in project channels / discussions.
