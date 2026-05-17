# Publishing to crates.io

This workspace ships three crates in dependency order:

```
rustdata-macros  →  rustdata-migrations  →  rustdata-core
```

`migration-test` is an integration-test crate and must **not** be published.

---

## Prerequisites

- A [crates.io](https://crates.io) account.
- A valid [`CARGO_REGISTRY_TOKEN`](https://crates.io/settings/tokens) stored as a GitHub Actions secret (repo → Settings → Secrets and variables → Actions).

---

## Option 1 — GitHub Actions (recommended)

The CI pipeline is defined in `.github/workflows/release.yml`. It requires no local tooling beyond git.

### Trigger a release

From GitHub → **Actions** → **rustdata-release** → **Run workflow**:

| Field | Value |
|---|---|
| **bump** | `patch` / `minor` / `major` |

This runs the **release** job (manual dispatch only, after `validate` passes):

1. **Compute next version** — reads `crates/rustdata-core/Cargo.toml`, bumps `major.minor.patch`
2. **Bump workspace version** — `cargo set-version --workspace $VER`
3. **Update `CHANGELOG.md`** — injects a `## [x.y.z] - YYYY-MM-DD` section
4. **Git commit + tag + push** — annotated tag (not signed) pushed to `origin` and `rustdata` remotes
5. **`cargo publish`** — all 3 crates in dependency order (`macros` → `migrations` → `core`), using `--allow-dirty`
6. **Create GitHub Release** — annotated release from the tag, body = new changelog section

The **validate** job runs automatically on every push/PR to `master` and runs `fmt` + `clippy` + `test` + `cargo publish --dry-run` (all 3 crates).

---

## Option 2 — `scripts/release.sh` (local)

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
| 3 | `make bump-version VER=…` | `cargo set-version --workspace` to synchronise all 3 crates |
| 4 | `make changelog` | Appends `## [x.y.z] - YYYY-MM-DD` to `CHANGELOG.md` if not already present |
| 5 | `make tag` | `git commit` + **GPG-signed** `git tag` + `git push --follow-tags` to `origin` and `rustdata` remotes |
| 6 | `make publish-upload` | `cargo publish` for all 3 crates in dependency order |

Requires `python3`, `git` (with GPG/SSH tag signing), and `cargo-set-version` (`cargo install cargo-set-version`).

---

## Option 3 — `make` targets (manual, step-by-step)

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
- [ ] Confirm the GitHub Release (created by CI or `make tag`) is live.
- [ ] Announce in project channels / discussions.
