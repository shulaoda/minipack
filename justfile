set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

setup:
    just check-setup-prerequisites
    # Rust related setup
    cargo install cargo-binstall
    cargo binstall taplo-cli cargo-insta cargo-deny cargo-shear -y
    # Node.js related setup
    corepack enable
    pnpm install
    just setup-bench
    @echo "✅✅✅ Setup complete!"

bench:
  pnpm --filter bench run bench

check-setup-prerequisites:
  node ./scripts/misc/setup-prerequisites/node.js

setup-bench:
  node ./scripts/misc/setup-benchmark-input/index.js