#!/bin/bash
set -euo pipefail

STATE_DIR="${STATE_DIR:-$HOME/.mempalace/hook_state}"
MEMPAL_DIR="${MEMPAL_DIR:-}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

run_mempalace() {
    if command -v mempalace >/dev/null 2>&1; then
        command mempalace "$@"
    elif [ -x "$REPO_DIR/target/release/mempalace" ]; then
        "$REPO_DIR/target/release/mempalace" "$@"
    elif [ -x "$REPO_DIR/target/debug/mempalace" ]; then
        "$REPO_DIR/target/debug/mempalace" "$@"
    else
        cargo run --quiet --manifest-path "$REPO_DIR/Cargo.toml" --bin mempalace -- "$@"
    fi
}

mkdir -p "$STATE_DIR"
ARGS=(hook precompact --state-dir "$STATE_DIR")
if [ -n "$MEMPAL_DIR" ]; then
    ARGS+=(--mempal-dir "$MEMPAL_DIR")
fi
run_mempalace "${ARGS[@]}"
