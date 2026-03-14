default:
    @just --list

setup-hooks:
    git config core.hooksPath .githooks

build:
    cargo build

check:
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    cargo fmt --all -- --check
    taplo check
    typos

fmt:
    cargo fmt --all
    taplo fmt

test:
    cargo test --workspace

lint:
    cargo clippy --workspace -- -D warnings

deny:
    cargo deny check

release-build:
    cargo build --release
