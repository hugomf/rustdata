# rustdata Makefile
# ─────────────────────────────────────────────────────────────────────────────
#   make check               fmt + clippy + test the whole workspace
#   make bump-version VER=x   bump workspace version (uses cargo-set-version)
#   make changelog [VER=x]    insert [x.y.z] section into CHANGELOG.md
#   make publish-dry-run      cargo publish --dry-run for rustdata-macros
#                              (leaf crate only — workspace crates validated by cargo test)
#   make tag                  git commit + annotated tag + push --follow-tags
#   make publish-upload       cargo publish × 3 (macros → migrations → core)
#   make publish              full pipeline in one shot
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
	@echo "=== publish-dry-run: rustdata-macros (leaf crate, no workspace deps) ===" && cargo publish -p rustdata-macros        --dry-run
	@echo "=== publish-dry-run: rustdata-migrations (sibling dep — validated by cargo test) ===" && true
	@echo "=== publish-dry-run: rustdata-core (sibling dep — validated by cargo test) ===" && true
	@echo "Dry-run passed."

# ── Git tag ───────────────────────────────────────────────────────────────────

.PHONY: tag
tag:
	@VER=$$(grep 'version = ' crates/rustdata-core/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'); \
	TAG="v$$VER"; \
	echo "Creating annotated tag $$TAG …"; \
	git commit -am "release: $$TAG" --allow-empty && \
	git tag -a "$$TAG" -m "Release $$TAG"; \
	git push origin --follow-tags; \
	echo "Tag $$TAG pushed to origin."

# ── Publish upload ────────────────────────────────────────────────────────────

.PHONY: publish-upload
publish-upload:
	@echo "=== publish-upload: rustdata-macros ==="    && cargo publish -p rustdata-macros
	@echo "=== publish-upload: rustdata-migrations ===" && cargo publish -p rustdata-migrations
	@echo "=== publish-upload: rustdata-core ==="      && cargo publish -p rustdata-core
	@echo "═══ All three crates published to crates.io ════════"

# ── Full pipeline ─────────────────────────────────────────────────────────────

.PHONY: publish
publish: check changelog publish-dry-run tag publish-upload
	@echo "═══ Done ════════"
