set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

setup:
    just check-setup-prerequisites
    # Rust related setup
    cargo install cargo-binstall
    cargo binstall cargo-deny cargo-shear -y
    # Node.js related setup
    corepack enable
    pnpm install
    just setup-bench
    @echo "✅✅✅ Setup complete!"

# Lint the codebase
lint: lint-rust lint-node lint-repo

lint-rust:
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo shear

lint-node:
    pnpm lint-code

lint-repo:
    pnpm lint-repo

bench:
  pnpm --filter bench run bench

check-setup-prerequisites:
  node ./scripts/misc/setup-prerequisites/node.js

setup-bench:
  node ./scripts/misc/setup-benchmark-input/index.js