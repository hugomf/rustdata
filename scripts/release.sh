#!/usr/bin/env bash
# ── scripts/release.sh ────────────────────────────────────────────────────────
#
# Full release workflow for rustdata crates on crates.io.
#
# Usage:
#   ./scripts/release.sh                      → interactive (prompts for bump type)
#   ./scripts/release.sh --bump minor --yes   → non-interactive minor bump
#   ./scripts/release.sh --dry-run            → validate, do not publish
#   ./scripts/release.sh --help               → show this usage
#
# Requires:
#   cargo, git (with gpg / ssh tag signing), cargo-release (optional)
#
# Steps:
#   1. cargo fmt && cargo clippy && cargo test
#   2. cargo publish --dry-run  (all crates, dependency order)
#   3. Bump version in Cargo.toml (semver)
#   4. Update CHANGELOG.md
#   5. git commit + sign-tag + push --follow-tags (all remotes)
#   6. cargo publish (all crates, dependency order)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DRY_RUN=0
BUMP_TYPE=""
ASSUME_YES=0

# ── Usage ─────────────────────────────────────────────────────────────────────

usage() {
  sed -n '/^### Usage/,/^###/p' "$SCRIPT_DIR/../README.md" |
    sed '/^###/d' |
    sed 's/^/// //' || true
  cat <<'EOF'
Usage:
  ./scripts/release.sh                 → interactive: asks which bump before proceeding
  ./scripts/release.sh --bump minor    → non-interactive minor bump
  ./scripts/release.sh --bump major    → non-interactive major bump
  ./scripts/release.sh --bump patch    → non-interactive patch bump
  ./scripts/release.sh --bump minor --yes  → non-interactive; does not prompt for confirmation
  ./scripts/release.sh --dry-run        → validate only (fmt, clippy, test, publish --dry-run)
  ./scripts/release.sh --help           → this message
EOF
}

# ── Parse args ────────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bump)
      BUMP_TYPE="${2:?missing value for --bump}"
      shift 2
      ;;
    --yes|-y)
      ASSUME_YES=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

cd "$REPO_ROOT"

# ── Detect current version ────────────────────────────────────────────────────

CURRENT_VER=$(grep '^version = ' crates/rustdata-core/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
PACKAGES="rustdata-macros rustdata-migrations rustdata-core"

echo "═══ rustdata release script ═══════════════════════════════════"
echo "  current version : $CURRENT_VER"
echo "  dry-run         : $DRY_RUN"
echo "══════════════════════════════════════════════════════════════"

# ── Confirm bump type ────────────────────────────────────────────────────────

if [[ -z "$BUMP_TYPE" ]]; then
  echo ""
  echo "Select bump type:"
  echo "  1) major  (0.x.0 → 1.0.0)"
  echo "  2) minor  (0.1.x → 0.2.0)"
  echo "  3) patch  (0.1.0 → 0.1.1)"
  read -rp "Enter [1-3]: " _choice
  case "$_choice" in
    1) BUMP_TYPE="major" ;;
    2) BUMP_TYPE="minor" ;;
    3) BUMP_TYPE="patch" ;;
    *) echo "Invalid choice."; exit 1 ;;
  esac
fi

case "$BUMP_TYPE" in
  major|minor|patch) ;;
  *) echo "Invalid --bump value: $BUMP_TYPE (need major|minor|patch)"; exit 1 ;;
esac

# ── Compute next version with Python semver ───────────────────────────────────

NEXT_VER=$(python3 - <<PY "$CURRENT_VER" "$BUMP_TYPE"
import sys
major, minor, patch = map(int, sys.argv[1].split("."))
bump = sys.argv[2]
if   bump == "major": major += 1; minor = 0; patch = 0
elif bump == "minor":          minor += 1; patch = 0
elif bump == "patch":                         patch += 1
print(f"{major}.{minor}.{patch}")
PY
)

echo "  bump            : $BUMP_TYPE  ($CURRENT_VER → $NEXT_VER)"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo ""
  echo "── DRY RUN ── step 1: quality gate ──"
fi

# ── Step 1: Quality gate ──────────────────────────────────────────────────────

make check

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo ""
  echo "── DRY RUN ── step 2: publish --dry-run ──"
  make publish-dry-run
  echo ""
  echo "── DRY RUN complete. No files were modified. ──"
  exit 0
fi

# ── Confirm ─────────────────────────────────────────────────────────────────

if [[ "$ASSUME_YES" -ne 1 ]]; then
  read -rp "Proceed with release v$NEXT_VER? [y/N] " _confirm
  if [[ ! "$_confirm" =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
  fi
fi

# ── Step 2: Bump version ─────────────────────────────────────────────────────

make bump-version VER="$NEXT_VER"

# ── Step 3: Update changelog ─────────────────────────────────────────────────

make changelog

# ── Step 4: Commit + sign-tag + push ────────────────────────────────────────

make tag

# ── Step 5: cargo publish (all crates in dependency order) ──────────────────

make publish-upload

echo ""
echo "═══ Release v$NEXT_VER complete ═══════════════════════════════════"
echo "  crates.io   : https://crates.io/crates/rustdata-core"
echo "  changelog   : $CHANGELOG_FILE"
echo "═══════════════════════════════════════════════════════════════"
