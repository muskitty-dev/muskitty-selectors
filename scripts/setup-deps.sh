#!/bin/bash
# Clone path dependencies for independent CI.
#
# muskitty-selectors uses path dependencies for local development within the
# MusKitty workspace. When this repo is cloned standalone (e.g. on CI), the
# path deps must be materialized at the expected relative locations.
#
# Dependency chain:
#   muskitty-selectors → muskitty-css (path) → muskitty-css-parser (path) → muskitty-css-tokenizer (path)
#   muskitty-selectors [dev-dep] → muskitty-dom (path)
#
# All path deps are independent repos under muskitty-dev/.
#
# Idempotent: skips clones that already exist (useful for local re-runs).
set -euo pipefail

clone_if_absent() {
    local url="$1"
    local dest="$2"
    if [ -d "$dest" ]; then
        echo "skip $dest (exists)"
    else
        git clone --depth 1 "$url" "$dest"
    fi
}

clone_if_absent https://github.com/muskitty-dev/muskitty-css.git ../muskitty-css
clone_if_absent https://github.com/muskitty-dev/muskitty-css-parser.git ../muskitty-css-parser
clone_if_absent https://github.com/muskitty-dev/muskitty-css-tokenizer.git ../muskitty-css-tokenizer
clone_if_absent https://github.com/muskitty-dev/muskitty-dom.git ../muskitty-dom
