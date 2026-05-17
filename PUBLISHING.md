# Publishing to crates.io

This workspace ships three crates in dependency order:

```
rustdata-macros  →  rustdata-migrations  →  rustdata-core
```

`migration-test` is an integration-test crate and must **not** be published.

---

## Prerequisites

- A [crates.io](https://crates.io) account.
- A valid [`CARGO_REGISTRY_TOKEN`](https://crates.io/settings/tokens) stored as an Actions secret in the repo (GitHub: Settings → Secrets and variables → Actions; Gitea: Settings → Actions → Secrets).

---

## CI pipelines

The project has two nearly identical CI pipelines. One triggers on GitHub, the other on Gitea. Both have the same two jobs.

### `validate` job (every push + PR)

Runs automatically on every push to `master` and on every PR. Steps:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --workspace --all-features`
4. `cargo publish --dry-run` for **all 3 crates** in dependency order

### `release` job (manual dispatch only)

Runs only on manual trigger, after `validate` passes. Same steps regardless of which CI you use:

| Step | GitHub Actions | Gitea Actions |
|---|---|---|
| 1 — Compute next version | `actions/github-script@v7` (JavaScript `bump()`) | `scripts/bump_version.sh` (Python) |
| 2 — Bump workspace version | `cargo set-version --workspace $VER` | `cargo set-version --workspace $VER` |
| 3 — Update `CHANGELOG.md` | shell script (injects below `# CHANGELOG` header) | shell script (prepends to file) |
| 4 — Git commit + tag + push | `git commit` + `git tag -a` + `git push --follow-tags` to `origin` and `rustdata` | same |
| 5 — `cargo publish` | all 3 crates with `--allow-dirty` | all 3 crates with `--allow-dirty` |
| 6 — Create release | `softprops/action-gh-release@v1` (GitHub Release from tag) | *(not configured — step is a no-op)* |

---

## Option A — GitHub Actions

Trigger from: **GitHub → Actions → rustdata-release → Run workflow**

| Field | Value |
|---|---|
| **bump** | `patch` / `minor` / `major` |

The GitHub workflow uses `actions/github-script@v7` for version bumping and `softprops/action-gh-release@v1` to create a GitHub Release when done.

---

## Option B — Gitea Actions

Trigger from: **Gitea → Actions → rustdata-release → Run**

Gitea uses `BUMP_TYPE` as an environment variable (default: `patch`) rather than a dropdown input. The GitHub Release creation step is present but disabled (`#177`). Modify it to point at the Gitea Releases API if you want a release note to appear on Gitea.

---

## Option C — `scripts/release.sh` (local)

`scripts/release.sh` is the local full-automation release script. It calls the Makefile targets in sequence.

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

What it does:

| Step | Command | Description |
|---|---|---|
| 1 | `make check` | `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test --workspace --all-features` |
| 2 | `make publish-dry-run` | `cargo publish --dry-run` for all 3 crates in dependency order |
| 3 | `make bump-version VER=…` | `cargo set-version --workspace` via `cargo-set-version` |
| 4 | `make changelog` | Appends `## [x.y.z] - YYYY-MM-DD` to `CHANGELOG.md` if not already present |
| 5 | `make tag` | `git commit` + **GPG-signed** `git tag` + `git push --follow-tags` to `origin` and `rustdata` remotes |
| 6 | `make publish-upload` | `cargo publish` for all 3 crates in dependency order |

Requires `python3`, `git` (with GPG/SSH tag signing), and `cargo-set-version` (`cargo install cargo-set-version`).

---

## Option D — `make` targets (manual, step-by-step)

```bash
# 1. Quality gate
make check

# 2. Dry-run publish (previews .crate files, does not upload)
make publish-dry-run

# 3. Bump workspace version
make bump-version VER=0.2.0

# 4. Add [x.y.z] section to CHANGELOG.md
make changelog

# 5. Commit + GPG-signed tag + push
make tag

# 6. Upload to crates.io (all 3 crates in order)
make publish-upload
```

### `make publish`

Runs the full local pipeline (check → changelog → publish-dry-run → tag → publish-upload):

```bash
make publish
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
- [ ] Confirm the GitHub Release (created by CI) is live.
- [ ] Announce in project channels / discussions.
