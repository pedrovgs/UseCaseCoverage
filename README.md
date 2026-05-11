# 🚀 Use Case Coverage (UCC)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)

🛡️ **Use Case Coverage** is a powerful developer tool designed to bridge the gap between feature specifications and test implementations. It helps you track, calculate, and visualize your test coverage against declared use cases, features, and bugs using a simple yet strict `.ucc` YAML format. 

🤖 **AI-Ready Quality Assurance**: In the age of AI-driven development, tracking the coverage of your AI agents is vital for maintaining software excellence. **UseCaseCoverage (UCC)** empowers you to measure that impact with precision and ease. ✨

---

### 🖼️ Screenshots

| Dashboard | Inventory |
| :---: | :---: |
| ![Dashboard](art/1%20dashboard.png) | ![Inventory](art/2%20inventory.png) |

| Feature Detail | Coverage Gaps |
| :---: | :---: |
| ![Feature Detail](art/3%20feature-detail.png) | ![Coverage Gaps](art/4%20coverage-gaps.png) |

| Tags |
| :---: |
| ![Tags](art/5%20tags.png) |

---

🧞 **Privacy First & Zero Infrastructure**: UseCaseCoverage is designed to run entirely on your local machine or within your CI pipeline. There is **no backend**, no data is ever uploaded to any server, and your specifications remain completely private and secure. Since it is a standalone CLI tool, there is no infrastructure to host or maintain.


## 📦 Installation

Assuming you have [Homebrew](https://brew.sh/) installed:

```bash
brew tap pedrovgs/tap
brew install ucc
```

Alternatively, you can install it in a single command:
```bash
brew install pedrovgs/tap/ucc
```

---

## 🛠 Usage

Tracking your coverage with UCC follows a simple three-step workflow:

1. 📝 **Define your Specs**: Write your features, use cases, bugs, and regressions in the simple `.ucc` YAML format anywhere in your project.
2. 🏷️ **Annotate your Tests**: Include the unique ID of your artifacts in your test code (as a comment or within a string). UCC scans your codebase to find these matches automatically. **You can annotate any type of test even in monorepos using different proramming languages and frameworks.**
3. 🚀 **Generate & Analyze**: Run `ucc report` to compute coverage metrics and generate a beautiful, interactive dashboard. You can exeucte ``ucc`` locally or integrate it in your CI/CD pipeline.

---

### 🏷️ Annotation Examples

UCC is language-agnostic. You can annotate your tests by simply including the artifact ID anywhere in your test file (e.g., in a comment, test name, or metadata):

#### 🍎 Swift (XCTest / Swift Testing)
```swift
func testLoginFlow() {
    // ucc-feat-1
    let login = LoginView()
    ...
}
```

#### 🔷 TypeScript (Jest / Mocha / Vitest)
```typescript
test('should authenticate user (ucc-feat-1)', () => {
    const auth = new AuthService();
    ...
});
```

#### 🦀 Rust (Cargo Test)
```rust
#[test]
fn test_artifact_parsing() {
    // ucc-feat-1
    let parser = YamlUccParser;
    ...
}
```

#### ☕ Kotlin (JUnit / Spek)
```kotlin
@Test
fun testSecureLogin() {
    /* ucc-feat-1 */
    val security = SecurityManager()
    ...
}
```

---

## 📄 The `.ucc` Format

UseCaseCoverage relies on `.ucc` files—standard YAML files that live alongside your code.

### ✨ Basic Structure

```yaml
schema_version: "1.0"

feature:
  id: user-authentication
  title: User Authentication
  created_at: "2026-05-10"
  description: >
    Handles user login, signup, and session management.

related_features: []

artifacts:
  - id: ucc-feat-1
    title: Successful login with valid credentials
    priority: high
    created_at: "2026-05-10"
    steps:
      - Enter username
      - Enter password
      - Click submit
    expected:
      - User is redirected to dashboard

  - id: ucc-feat-2
    type: bug
    title: Login fails on slow connections
    priority: highest
    created_at: "2026-05-10"
    related: [ucc-auth-001]
    tags: [auth, secure]
    platforms: [web, android, ios]
    coverage_gap_reason: "Impossible to test on Android automatically"
```

### ✅ Linting `.ucc` files

Ensure your specifications are perfect before generating reports. The `lint` command validates your `.ucc` files recursively.

```bash
ucc lint
```

> **Tip:** You can specify an input directory or an output file for automated audits:
> ```bash
> ucc lint --input ./specs --output lint-results.txt
> ```

### 📊 Generating Visual Reports

Transform your YAML files into a stunning, interactive HTML dashboard that visualizes your project's health.

```bash
ucc report
```

By default, reports are neatly organized in `.ucc/reports/<YYYY-MM-DD>`. 

---

### 🧩 Key Components

- **Use Cases**: Standard artifacts capturing user requirements.
- **Bugs & Regressions**: Explicitly tracked items with `type: bug` or `type: regression`.
- **Cross-Platform Tracking**: Granularly track coverage across `android`, `apple`, `web`, `windows`, or any other platform you define in the `platforms` field.
- **Coverage Gaps**: Document known missing coverage with `coverage_gap_reason` to keep the team informed about missing automated coverage.

---

## 🤖 GitHub Actions Integration

Automate your coverage tracking with a simple workflow. Ensure every PR maintains high documentation standards!

```yaml
name: Use Case Coverage
on: [push]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install ucc
        run: brew install pedrovgs/tap/ucc
        
      - name: Lint UCC files
        run: ucc lint
        
      - name: Generate Report
        run: ucc report
        
      - name: Upload Report
        uses: actions/upload-artifact@v4
        with:
          name: ucc-report
          path: .ucc/reports/
```

---

## 👨‍💻 Developer Guide

### 🛠 Prerequisites

- Rust stable toolchain (`rustup` + `cargo`)
- Git

### 🚀 Initial Setup

```bash
rustup toolchain install stable
rustup default stable
rustup component add rustfmt clippy
```

### 🧪 Workspace Commands

| Command | Action |
| :--- | :--- |
| `cargo fmt --all` | Format the entire codebase |
| `cargo clippy --workspace` | Run the linter |
| `cargo test --workspace` | Run all unit and e2e tests |
| `cargo run -p use_case_coverage_cli -- report` | Run the CLI locally |

---

## Developed By
------------

* Pedro Vicente Gómez Sánchez - <pedrovicente.gomez@gmail.com>

<a href="https://x.com/pedro_g_s">
  <img alt="Follow me on X" src="https://img.icons8.com/?size=100&id=6Fsj3rv2DCmG&format=png&color=000000" height="60" width="60"/>
</a>
<a href="https://es.linkedin.com/in/pedrovgs">
  <img alt="Add me to Linkedin" src="https://img.icons8.com/?size=100&id=447&format=png&color=000000" height="60" width="60"/>
</a>

---

## 📄 License

```text
Copyright 2026 Pedro Vicente Gómez Sánchez

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
