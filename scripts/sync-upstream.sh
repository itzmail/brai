#!/usr/bin/env bash
# Sync upstream zeroclaw changes into brai.
# Usage: ./scripts/sync-upstream.sh [--dry-run]
#
# Flow:
#   1. Ensure 'upstream' remote points to zeroclaw
#   2. Fetch upstream
#   3. Detect new commits since last sync tag
#   4. Create branch sync/upstream-<date>
#   5. Attempt merge; if conflict → leave unresolved, instruct user to open PR manually
#   6. If clean → open PR via gh (if available)

set -euo pipefail

UPSTREAM_URL="https://github.com/zeroclaw-labs/zeroclaw"
UPSTREAM_REMOTE="upstream"
BASE_BRANCH="main"
SYNC_TAG_PREFIX="sync/last-upstream-"
DATE=$(date +%Y%m%d-%H%M)
SYNC_BRANCH="sync/upstream-${DATE}"
DRY_RUN=false

[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=true

log()  { echo "[sync] $*"; }
warn() { echo "[warn] $*" >&2; }
die()  { echo "[error] $*" >&2; exit 1; }

# Guard: must be on base branch with clean working tree
current_branch=$(git branch --show-current)
[[ "$current_branch" == "$BASE_BRANCH" ]] || die "Must run from '$BASE_BRANCH' branch (currently on '$current_branch')"
[[ -z "$(git status --porcelain)" ]] || die "Working tree not clean — commit or stash first"

# Ensure upstream remote exists
if ! git remote get-url "$UPSTREAM_REMOTE" &>/dev/null; then
    log "Adding remote '$UPSTREAM_REMOTE' → $UPSTREAM_URL"
    git remote add "$UPSTREAM_REMOTE" "$UPSTREAM_URL"
fi

log "Fetching $UPSTREAM_REMOTE ..."
git fetch "$UPSTREAM_REMOTE" --no-tags --depth=50 --quiet

upstream_head=$(git rev-parse "$UPSTREAM_REMOTE/$BASE_BRANCH")
log "Upstream HEAD: ${upstream_head:0:12}"

# Find base commit: last sync tag or common ancestor
last_sync_tag=$(git tag --list "${SYNC_TAG_PREFIX}*" --sort=-version:refname | head -1)
if [[ -n "$last_sync_tag" ]]; then
    base_commit=$(git rev-parse "$last_sync_tag")
    log "Last sync tag: $last_sync_tag (${base_commit:0:12})"
else
    base_commit=$(git merge-base HEAD "$upstream_head")
    log "No sync tag found; using merge-base: ${base_commit:0:12}"
fi

new_commits=$(git log --oneline "${base_commit}..${upstream_head}" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$new_commits" -eq 0 ]]; then
    log "Already up to date with upstream. Nothing to sync."
    exit 0
fi

log "$new_commits new commit(s) from upstream:"
git log --oneline "${base_commit}..${upstream_head}"

if $DRY_RUN; then
    log "[dry-run] Would create branch '$SYNC_BRANCH' and merge upstream. Exiting."
    exit 0
fi

# Create sync branch off current base
git checkout -b "$SYNC_BRANCH"
log "Created branch: $SYNC_BRANCH"

# Attempt merge
set +e
git merge "$upstream_head" --no-edit -m "sync: merge upstream zeroclaw @ ${upstream_head:0:12}"
merge_exit=$?
set -e

if [[ $merge_exit -ne 0 ]]; then
    warn "Merge has conflicts. Files with conflicts:"
    git diff --name-only --diff-filter=U
    echo ""
    warn "Resolve conflicts, commit, then push and open a PR:"
    warn "  git add <resolved-files>"
    warn "  git commit"
    warn "  git push origin $SYNC_BRANCH"
    warn "  gh pr create --base $BASE_BRANCH --head $SYNC_BRANCH --title 'sync: upstream zeroclaw ${DATE} (manual resolve)'"
    exit 1
fi

# Tag the synced commit for future runs
tag_name="${SYNC_TAG_PREFIX}${DATE}"
git tag "$tag_name"
log "Tagged sync point: $tag_name"

git push origin "$SYNC_BRANCH"
git push origin "$tag_name"
log "Pushed branch and tag"

# Open PR if gh is available
if command -v gh &>/dev/null; then
    pr_url=$(gh pr create \
        --base "$BASE_BRANCH" \
        --head "$SYNC_BRANCH" \
        --title "sync: upstream zeroclaw ${DATE}" \
        --body "$(cat <<EOF
## Upstream Sync — ${DATE}

Merges **${new_commits}** commit(s) from [zeroclaw-labs/zeroclaw](${UPSTREAM_URL}).

### New upstream commits
\`\`\`
$(git log --oneline "${base_commit}..${upstream_head}")
\`\`\`

### Checklist
- [ ] Review brai-specific customizations still intact
- [ ] No \`zeroclaw\` references introduced in new code
- [ ] Tests pass
EOF
        )")
    log "PR opened: $pr_url"
else
    log "gh CLI not found — push done, open PR manually:"
    log "  gh pr create --base $BASE_BRANCH --head $SYNC_BRANCH"
fi

log "Done."
