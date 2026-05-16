# rustdata Makefile
# ─────────────────────────────────────────────────────────────────────────────
#   INFO
#     make check               fmt + clippy + test the whole workspace
#     make bump-version VER=x   bump workspace version (uses cargo-set-version)
#     make changelog [VER=x]    append or update the [x.y.z] section in CHANGELOG.md
#     make publish-dry-run      cargo publish --dry-run (all 3 crates in order)
#     make tag                  git commit + GPG-sign tag vX.Y.Z + push --follow-tags
#     make publish              full release (check → changelog → dry-run → tag → publish)
# ─────────────────────────────────────────────────────────────────────────────

export TZ := UTC

PACKAGES   := rustdata-macros rustdata-migrations rustdata-core
CHANGELOG  := CHANGELOG.md

# ── Quality gate ─────────────────────────────────────────────────────────────

.PHONY: check
check:
	@echo "── fmt ──────────────────────────────────────────────"
	cargo fmt --all -- --check
	@echo "── clippy ───────────────────────────────────────────"
	cargo clippy --all-targets --all-features -- -D warnings
	@echo "── test ─────────────────────────────────────────────"
	cargo test --workspace --all-features
	@echo "All quality checks passed."

# ── Version bump ──────────────────────────────────────────────────────────────

.PHONY: bump-version
bump-version:
	@test -n "$(VER)" || { echo "Usage: make bump-version VER=0.2.0"; exit 1; }
	@echo "Bumping workspace to $(VER)…"
	@cargo set-version --workspace "$(VER)"
	@cargo metadata --format-version=1 --no-deps \
		| python3 -c "import sys,json;\
			print(', '.join(f\"{p['name']}={p['version']}\" \
				for p in json.load(sys.stdin)['packages'] \
				if 'rustdata' in p['name']))"

# ── Changelog ─────────────────────────────────────────────────────────────────

.PHONY: changelog
changelog:
	@VER=$$(grep 'version = ' crates/rustdata-core/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'); \
	VER="${VER}$${VER:+-$${1}}" ; \
	if grep -q "^## \[$$VER\]" "$(CHANGELOG)"; then \
		echo "Changelog already has an entry for [$$VER] — skipping."; \
	else \
		DATE=$$(date +%Y-%m-%d); \
		PREAMBLE=$$(sed -n '/^# /,/^## \[/p' "$(CHANGELOG)" | head -n -1); \
		if [ -n "$$PREAMBLE" ]; then \
			SECTION_BODY="$$PREAMBLE\n"; \
		else \
			SECTION_BODY=""; \
		fi; \
		SECTION_BODY="$$SECTION_BODY\n## [$$VER] - $$DATE\n\n### Added\n\n### Changed\n\n### Fixed\n\n"; \
		REST=$$(grep -av '^# ' "$(CHANGELOG)" | cat); \
		printf "%b%b" "$$SECTION_BODY" "$$REST" > "$(CHANGELOG).tmp"; \
		mv "$(CHANGELOG).tmp" "$(CHANGELOG)"; \
		echo "Changelog updated."; \
	fi

# ── Publish dry-run ──────────────────────────────────────────────────────────

.PHONY: publish-dry-run
publish-dry-run:
	@echo "=== publish-dry-run: rustdata-macros ===" && cargo publish -p rustdata-macros        --dry-run
	@echo "=== publish-dry-run: rustdata-migrations ===" && cargo publish -p rustdata-migrations    --dry-run
	@echo "=== publish-dry-run: rustdata-core ===" && cargo publish -p rustdata-core          --dry-run
	@echo "All dry-runs passed."

# ── Git tag ───────────────────────────────────────────────────────────────────

.PHONY: tag
tag:
	@VER=$$(grep 'version = ' crates/rustdata-core/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'); \
	TAG="v$$VER"; \
	echo "Creating signed tag $$TAG …"; \
	git commit -am "release: $$TAG" --allow-empty && \
	git tag -s "$$TAG" -m "Release $$TAG"; \
	git push origin --follow-tags && \
	git push rustdata --follow-tags && \
	echo "Tag $$TAG pushed to origin and rustdata."

# ── Publish upload only (no re-run of check/changelog/dry-run/tag) ───────────

.PHONY: publish-upload
publish-upload:
	@echo "=== publish-upload: rustdata-macros ==="    && cargo publish -p rustdata-macros
	@echo "=== publish-upload: rustdata-migrations ===" && cargo publish -p rustdata-migrations
	@echo "=== publish-upload: rustdata-core ==="      && cargo publish -p rustdata-core
	@echo "═══ All three crates published to crates.io ════════"

# ── Full publish ──────────────────────────────────────────────────────────────

.PHONY: publish
publish: check changelog publish-dry-run tag publish-upload
	@echo "═══ Done ════════"
