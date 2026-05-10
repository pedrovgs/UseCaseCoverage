# Use Case Coverage

This project helps you track and calculate your test implementation coverage against declared use cases and features using `.ucc` files.

## The `.ucc` Format

UseCaseCoverage relies on `.ucc` files. These files use strict YAML to declare an application feature along with its related use cases and bugs. You can place `.ucc` files anywhere in your project; UseCaseCoverage will discover them recursively.

### Basic Structure

Every `.ucc` file must define `schema_version` and a `feature`. Inside the `feature`, you list a set of `artifacts` (such as use cases or bugs).

```yaml
schema_version: "1.0"

feature:
  id: user-authentication
  title: User Authentication
  created_at: "2026-05-10"
  updated_at: "2026-05-10"
  description: >
    Handles user login, signup, and session management.

tags:
  - security
  - core

platforms:
  - web
  - android
  - ios

related_features: []

artifacts:
  - id: ucc-auth-001
    created_at: "2026-05-10"
    title: Successful login with valid credentials
    priority: high
    steps:
      - Enter username
      - Enter password
      - Click submit
    expected:
      - User is redirected to dashboard

  - id: bug-auth-001
    type: regression
    created_at: "2026-05-10"
    title: Login fails on slow connections
    severity: high
    status: open
    related:
      - ucc-auth-001
```

### Artifacts (Use Cases and Bugs)

In the `artifacts` array, you declare the specific use cases you want to verify in your tests, or bugs associated with the feature.
- **Use Cases** are standard artifacts without an explicit `type` (or `type` omitted).
- **Bugs** declare an explicit `type` like `bug` or `regression`, and can optionally include `severity`, `status`, and `related` list (to link back to the use case they broke).

## Usage

You can spread multiple `.ucc` files anywhere across your codebase (e.g., nested inside your domain or e2e folders). 

1. Write your `.ucc` scenarios to capture the feature specifications.
2. In your integration or end-to-end tests, map these use cases to code.
3. The tool will parse scenarios, discover matched cases within your tests, and compute your use case coverage seamlessly.

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
   *To re-record `insta` snapshot tests if your changes intentionally modify output schemas (like adding new `.ucc` scenarios or modifying parsers), run:*
   ```bash
   INSTA_UPDATE=always cargo test
   ```
   *Alternatively, if you have `cargo-insta` installed, you can interactively review and accept snapshots with `cargo insta review`.*
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
