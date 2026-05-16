#!/usr/bin/env bash
# scripts/bump_version.sh — read the CURRENT arg, bump by TYPE (major|minor|patch),
# print NEXT to stdout
set -euo pipefail
CURRENT="${1:?Usage: bump_version.sh 0.1.0 major|minor|patch}"
TYPE="${2:?Usage: bump_version.sh 0.1.0 major|minor|patch}"

python3 - "$CURRENT" "$TYPE" <<'PY'
import sys
major, minor, patch = map(int, sys.argv[1].split("."))
bump = sys.argv[2]
if   bump == "major": major += 1; minor = 0; patch = 0
elif bump == "minor":          minor += 1; patch = 0
elif bump == "patch":                         patch += 1
print(f"{major}.{minor}.{patch}")
PY
