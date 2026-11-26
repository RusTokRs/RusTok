#!/usr/bin/env bash
set -euo pipefail

cargo test -p xtask --locked
cargo xtask module verify-scaffold "$@"
git diff --check
