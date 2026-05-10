# Developer Instructions

## Prerequisites

- Rust stable toolchain (`rustup` + `cargo`)
- Git

Optional (used in CI and recommended locally):

- `cargo-mutants` for mutation testing
- `cargo-llvm-cov` for coverage reports

## Initial Setup

1. Ensure Rust stable is installed:
   ```bash
   rustup toolchain install stable
   rustup default stable
   ```
2. Install formatter and linter components:
   ```bash
   rustup component add rustfmt clippy
   ```
3. (Optional) Install extra quality tools:
   ```bash
   cargo install cargo-mutants --locked
   cargo install cargo-llvm-cov --locked
   ```

## Workspace Commands

Run from repository root.

1. Format:
   ```bash
   cargo fmt --all
   ```
2. Lint:
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
3. Test:
   ```bash
   cargo test --workspace --all-targets --all-features
   ```
4. Mutation testing (quick listing mode):
   ```bash
   cargo mutants --workspace --list --no-times
   ```
5. Coverage report:
   ```bash
   cargo llvm-cov --workspace --all-features --lcov --output-path coverage/lcov.info
   ```

## Running the CLI

Build and run locally:

```bash
cargo run -p use_case_coverage_cli -- 3 5
```

Build release binary:

```bash
cargo build --release -p use_case_coverage_cli
```

## CI Workflows

- `CI` workflow validates formatting, linting, tests, mutation tooling smoke step, and coverage generation.
- `Build UseCaseCoverageCLI` workflow builds the CLI release binary and uploads an archived artifact.

## Suggested Daily Flow

1. Pull latest changes.
2. Implement changes.
3. Run `cargo fmt --all`.
4. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. Run `cargo test --workspace --all-targets --all-features`.
6. Commit only after all checks pass locally.
