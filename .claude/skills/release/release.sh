#!/usr/bin/env bash
# logzip release helper — syncs the version across all 5 points, runs the test
# suites, updates CHANGELOG, commits, tags, and (optionally) pushes + monitors CI.
#
# Usage:
#   release.sh <X.Y.Z>            # sync + test + commit + tag locally (no push)
#   release.sh <X.Y.Z> --push     # ...and push main + tag, then monitor publish.yml
#   release.sh <X.Y.Z> --dry-run  # show what would change; touch nothing
#
# The 5 version points (see CLAUDE.md §VII):
#   Cargo.toml           [workspace.package] version  AND  logzip-core dep version
#   pyproject.toml       version
#   python/logzip/__init__.py   __version__
#   crates/logzip-py/src/lib.rs m.add("__version__", ...)
set -euo pipefail

NEW="${1:-}"
MODE="${2:-}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

[[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "error: version must be X.Y.Z, got '$NEW'"; exit 1; }

# Current version comes from the single source of truth: [workspace.package].
OLD="$(grep -m1 -E '^version = "[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
echo "Release: $OLD -> $NEW"
[[ "$OLD" != "$NEW" ]] || { echo "note: version unchanged — re-tagging the same version is a maintainer decision; do it by hand (see SKILL.md)."; exit 1; }

FILES=(Cargo.toml pyproject.toml python/logzip/__init__.py crates/logzip-py/src/lib.rs)

if [[ "$MODE" == "--dry-run" ]]; then
  echo "--- version strings that would change ($OLD -> $NEW) ---"
  grep -rn "\"$OLD\"" "${FILES[@]}" || true
  exit 0
fi

# Mechanical sync: replace the exact quoted version string in each file.
# Cargo.toml legitimately holds it twice (package + dependency); both must move.
sed -i -E "s/\"$OLD\"/\"$NEW\"/g" Cargo.toml
sed -i -E "s/^version = \"$OLD\"/version = \"$NEW\"/" pyproject.toml
sed -i -E "s/__version__ = \"$OLD\"/__version__ = \"$NEW\"/" python/logzip/__init__.py
sed -i -E "s/(m\.add\(\"__version__\", )\"$OLD\"/\1\"$NEW\"/" crates/logzip-py/src/lib.rs

# Guard: no stale version may remain in any of the 5 points.
if grep -rn "\"$OLD\"" "${FILES[@]}"; then
  echo "error: stale version still present above — aborting, fix by hand."; exit 1
fi
echo "✓ version synced in all 5 points"

# Promote the CHANGELOG [Unreleased] block to the new version, if present.
if grep -q '^## \[Unreleased\]' CHANGELOG.md; then
  sed -i -E "s/^## \[Unreleased\]/## [$NEW] - $(date +%F)/" CHANGELOG.md
  echo "✓ CHANGELOG [Unreleased] -> [$NEW]"
else
  echo "! no [Unreleased] block in CHANGELOG — add release notes by hand before pushing"
fi

echo "=== cargo test --workspace ==="
cargo test --workspace
echo "=== pytest ==="
uv run --with pytest python -m pytest tests/test_logzip.py -q

git add Cargo.toml pyproject.toml python/logzip/__init__.py crates/logzip-py/src/lib.rs CHANGELOG.md
git commit -q -m "chore(release): v$NEW"
git tag "v$NEW"
echo "✓ committed and tagged v$NEW"

if [[ "$MODE" != "--push" ]]; then
  echo "Not pushed. To release:  git push origin main && git push origin v$NEW"
  exit 0
fi

git push origin main
git push origin "v$NEW"
echo "✓ pushed main + v$NEW — monitoring publish.yml..."

# Wait for the run keyed to this tag, then report its conclusion.
sleep 8
RID="$(gh run list --workflow=publish.yml --limit 1 --json databaseId -q '.[0].databaseId')"
until [[ "$(gh run view "$RID" --json status -q .status)" == "completed" ]]; do sleep 15; done
gh run view "$RID" 2>&1 | grep -E '✓|✗|X ' || true
CONC="$(gh run view "$RID" --json conclusion -q .conclusion)"
echo "publish.yml: $CONC"
[[ "$CONC" == "success" ]] || exit 1
