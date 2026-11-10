set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# List the available recipes
default:
    @just --list --unsorted

# Run every check: prose, shell and the commit message checker
[group('checks')]
check: check-prose check-shell check-commit-message-self-test

# Check Markdown prose in the given files, or in every Markdown file when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-prose *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-prose" "$@"

# Run shellcheck on every shell script and on the shell inside every recipe
[group('checks')]
check-shell:
    @scripts/check-shell

# Check a commit message file for attribution, non-ASCII text, and subject format
[group('checks')]
[no-cd]
[no-exit-message]
check-commit-message file:
    @python3 -I "{{ justfile_directory() }}/scripts/check-commit-message" "$1"

# Check that the commit message checker accepts a clean message and rejects attribution
[group('checks')]
check-commit-message-self-test:
    #!/usr/bin/env bash
    set -euo pipefail
    work="$(mktemp -d)"
    trap 'rm -rf -- "${work}"' EXIT
    printf 'Add the safe compiler session\n\nThe body says why.\n' >"${work}/good"
    printf 'Add the safe compiler session\n\nCo-Authored-By: Someone <someone@example.com>\n' >"${work}/bad"
    python3 -I scripts/check-commit-message "${work}/good"
    if python3 -I scripts/check-commit-message "${work}/bad" >/dev/null; then
        printf 'check-commit-message accepted an attribution line\n' >&2
        exit 1
    fi
    printf 'check-commit-message self-test passed\n'

# Install the pre-commit, commit-msg and post-merge git hooks
[group('checks')]
hooks-install:
    pre-commit install

# Squash-merge BRANCH onto main from the main checkout, remove its worktree and branch, then push main
[group('git')]
merge BRANCH *ARGS:
    @scripts/merge-branch "$@"

# Remove the worktrees and branches that are already squash-merged into main
[group('git')]
worktree-prune:
    @scripts/worktree-prune
