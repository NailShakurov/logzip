---
name: release
description: Use when cutting a logzip release — bumping the version, tagging, and triggering the PyPI/crates.io publish pipeline. Use when the user says "release", "cut a release", "bump version", "ship vX.Y.Z", or asks to publish to PyPI/crates.io.
disable-model-invocation: true
---

# Release logzip

logzip's version lives in **5 places** (CLAUDE.md §VII). Miss one → broken release that can't be un-published. The mechanical sync + test + tag is automated; the judgment calls are below.

## Do this

```bash
# 1. Preview the version sync — touches nothing:
.claude/skills/release/release.sh <X.Y.Z> --dry-run

# 2. Confirm the diff and CHANGELOG [Unreleased] notes with the user.

# 3. Cut locally (sync 5 points, run cargo+pytest, commit, tag — NO push):
.claude/skills/release/release.sh <X.Y.Z>

# 4. Push ONLY after explicit user approval (see no-push rule):
.claude/skills/release/release.sh <X.Y.Z> --push
```

`--push` pushes `main` + `vX.Y.Z` and blocks until `publish.yml` finishes, then prints its conclusion.

## Judgment calls (not automated)

- **Never push without explicit approval.** The repo memory `feedback_no_push` is binding. Steps 1–3 are safe; step 4 needs a clear "go".
- **CHANGELOG.** The script promotes `## [Unreleased]` → `## [X.Y.Z] - <date>`. If there's no `[Unreleased]` block, write release notes first — don't ship a release with an empty changelog.
- **Re-tagging the same version** (republish to the same vX.Y.Z) is a separate path the script refuses on purpose. Only do it when intentionally re-running CI on an already-released version; `publish.yml` is idempotent (`skip-existing` + "already exists" guards), so it won't hard-fail. Do it by hand: `git tag -f vX.Y.Z <sha> && git push -f origin vX.Y.Z`.
- **CI is tag-triggered only.** Pushing `main` alone runs nothing. The release is the tag.

## If something fails

- Stale version after sync → script aborts and prints the offending line; fix by hand, don't force.
- Tests red → fix before tagging; never tag over failing tests (CLAUDE.md §VI).
- `publish.yml` red → inspect with `gh run view <id> --log-failed`; the version is already public on whatever job succeeded, so prefer a patch bump over re-tagging.
