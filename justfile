set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

init:
    # Rust related setup
    cargo install cargo-binstall
    cargo binstall cargo-shear -y
    # Node.js related setup
    corepack enable
    pnpm install
    just setup-bench
    @echo "✅✅✅ Setup complete!"

setup:
    cargo install --path ./crates/minipack_cli

setup-bench:
  node ./bench/misc/index.js

bench:
  pnpm --filter bench run bench

lint: lint-rust lint-node lint-repo

lint-rust:
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo fmt --all
    cargo shear

lint-node:
    pnpm lint-code

lint-repo:
    pnpm lint-repo