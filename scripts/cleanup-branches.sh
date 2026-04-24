#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DRY_RUN=true
FORCE=false
PATTERNS=()

usage() {
    cat <<'EOF'
Usage: cleanup-branches.sh [OPTIONS] [PATTERNS...]

Bulk-delete stale remote branches. Safe and idempotent.

Options:
  --run         Actually delete branches (default: dry-run)
  --force       Delete even unmerged branches (default: merged-only)
  --help        Show this help

Patterns (default if none specified):
  polecat/*     Polecat worktree branches
  tw-polecat/*  Tw-era polecat branches
  fix/*         Fix branches
  temp-*        Temporary branches
  test-*        Test branches
  final-*       Final branches
  merge-*       Merge branches

Examples:
  ./scripts/cleanup-branches.sh              # Dry-run all default patterns
  ./scripts/cleanup-branches.sh --run        # Actually delete merged branches
  ./scripts/cleanup-branches.sh --run --force # Delete ALL matching branches
  ./scripts/cleanup-branches.sh --run "polecat/*"  # Only polecat branches
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --run)   DRY_RUN=false; shift ;;
        --force) FORCE=true; shift ;;
        --help)  usage ;;
        -*)      echo "Unknown option: $1"; exit 1 ;;
        *)       PATTERNS+=("$1"); shift ;;
    esac
done

if [[ ${#PATTERNS[@]} -eq 0 ]]; then
    PATTERNS=(
        "polecat/*"
        "tw-polecat/*"
        "fix/*"
        "temp-*"
        "test-*"
        "final-*"
        "merge-*"
    )
fi

BRANCHES=()
for pattern in "${PATTERNS[@]}"; do
    while IFS= read -r branch; do
        [[ -z "$branch" ]] && continue
        BRANCHES+=("$branch")
    done < <(git for-each-ref --format="%(refname:short)" "refs/remotes/origin/$pattern" 2>/dev/null || true)
done

if [[ ${#BRANCHES[@]} -eq 0 ]]; then
    echo "No branches match the given patterns."
    exit 0
fi

echo "Found ${#BRANCHES[@]} matching branches."
echo ""

TO_DELETE=()
SKIP_UNMERGED=()

for branch in "${BRANCHES[@]}"; do
    short="${branch#origin/}"

    if git merge-base --is-ancestor "$branch" main 2>/dev/null; then
        TO_DELETE+=("$short")
    else
        if $FORCE; then
            TO_DELETE+=("$short")
        else
            SKIP_UNMERGED+=("$short")
        fi
    fi
done

echo "=== Branches to delete (${#TO_DELETE[@]}) ==="
for b in "${TO_DELETE[@]}"; do
    echo "  $b"
done

if [[ ${#SKIP_UNMERGED[@]} -gt 0 ]]; then
    echo ""
    echo "=== Skipped (unmerged, use --force) (${#SKIP_UNMERGED[@]}) ==="
    for b in "${SKIP_UNMERGED[@]}"; do
        echo "  $b"
    done
fi

if $DRY_RUN; then
    echo ""
    echo "[DRY RUN] No branches deleted. Use --run to execute."
    exit 0
fi

echo ""
echo "Deleting ${#TO_DELETE[@]} branches..."

for short in "${TO_DELETE[@]}"; do
    if git push origin --delete "$short" 2>/dev/null; then
        echo "  DELETED: $short"
    else
        echo "  FAILED:  $short (may already be gone)"
    fi
done

git remote prune origin 2>/dev/null
echo ""
echo "Pruned stale remote tracking refs."
echo "Done. Deleted ${#TO_DELETE[@]} branches."
