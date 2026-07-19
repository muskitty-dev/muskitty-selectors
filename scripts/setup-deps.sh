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
# Of these:
#   - muskitty-css-parser, muskitty-css-tokenizer, muskitty-dom are independent repos under muskitty-dev/
#   - muskitty-css is NOT an independent repo; it lives in the main MusKitty repo (Ink-dark/MusKitty).
#     We shallow-clone the main repo and move crates/muskitty-css into place.
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

clone_if_absent https://github.com/muskitty-dev/muskitty-css-parser.git ../muskitty-css-parser
clone_if_absent https://github.com/muskitty-dev/muskitty-css-tokenizer.git ../muskitty-css-tokenizer
clone_if_absent https://github.com/muskitty-dev/muskitty-dom.git ../muskitty-dom

# muskitty-css is not an independent repo; extract from main MusKitty repo.
if [ -d ../muskitty-css ]; then
    echo "skip ../muskitty-css (exists)"
else
    rm -rf ../MusKitty-main
    git clone --depth 1 https://github.com/Ink-dark/MusKitty.git ../MusKitty-main
    mv ../MusKitty-main/crates/muskitty-css ../muskitty-css
    rm -rf ../MusKitty-main
fi
