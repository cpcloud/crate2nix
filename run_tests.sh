#!/usr/bin/env bash

top="$(readlink -f "$(dirname "$0")")"

cd "$top"/crate2nix

../regenerate_cargo_nix.sh && ./cargo.sh test || {
    echo "==================" >&2
    echo "cargo test: FAILED" >&2
    exit 1
}

# Crude hack check if we have the right to push to the cache
file=~/.config/cachix/cachix.dhall
if test -f "$file" && grep -q '"eigenvalue"' "$file"; then
    echo "Pushing build artifacts to eigenvalue.cachix.org..." >&2
    # we filter for "rust_" to exclude some things that are in the
    # nixos cache anyways
    nix-store -q -R --include-outputs $(nix-store -q -d target/nix-result*) |\
     grep -e "-rust_" |\
     cachix push eigenvalue
fi
