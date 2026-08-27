#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust/Cargo bulunamadı." >&2
    exit 1
fi

if ! command -v node >/dev/null 2>&1; then
    echo "Web JavaScript söz dizimi kontrolü için Node.js gerekiyor." >&2
    exit 1
fi

cd "$project_dir"

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
node --check apps/easyreg-server/static/app.js
sh -n scripts/check.sh

echo "Tüm yerel kontroller geçti."
