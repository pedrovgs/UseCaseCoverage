#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use serde_json::{json, Value};
use use_case_coverage_core::coverage_percentage;
use use_case_coverage_core::domain::{ArtifactCoverageIndex, FeatureDocument, UccLintResult};

/// Builds a tiny human-readable coverage report.
#[must_use]
pub fn build_report(covered: u32, total: u32) -> String {
    let percentage = coverage_percentage(covered, total);
    format!("Use case coverage: {covered}/{total} ({percentage:.2}%)")
}

/// Generates a responsive report bundle (`HTML + CSS + TS + JS`) in `<root>/.ucc/`.
///
/// # Errors
///
/// Returns an error if report directory or files cannot be written.
pub fn generate_html_report(
    root: &Path,
    features: &[FeatureDocument],
    lint_results: &[UccLintResult],
    coverage_index: &ArtifactCoverageIndex,
) -> Result<(), std::io::Error> {
    let report_dir = root.join(".ucc");
    fs::create_dir_all(&report_dir)?;

    let report_data = build_report_data(features, lint_results, coverage_index);
    let report_json = serde_json::to_string_pretty(&report_data)
        .map_err(|error| std::io::Error::other(format!("JSON serialization failed: {error}")))?;

    fs::write(report_dir.join("index.html"), html_template())?;
    fs::write(report_dir.join("styles.css"), css_template())?;
    fs::write(report_dir.join("app.ts"), ts_template())?;
    fs::write(report_dir.join("app.js"), js_template())?;
    fs::write(report_dir.join("data.json"), report_json)?;

    Ok(())
}

fn build_report_data(
    features: &[FeatureDocument],
    lint_results: &[UccLintResult],
    coverage_index: &ArtifactCoverageIndex,
) -> Value {
    let mut total_use_cases = 0_u32;
    let mut total_use_cases_covered = 0_u32;
    let mut total_bugs = 0_u32;
    let mut total_bugs_covered = 0_u32;

    let feature_rows: Vec<Value> = features
        .iter()
        .map(|feature| {
            let mut feature_use_cases = 0_u32;
            let mut feature_use_cases_covered = 0_u32;
            let mut feature_bugs = 0_u32;
            let mut feature_bugs_covered = 0_u32;

            for artifact in &feature.artifacts {
                let covered = coverage_index.is_covered(&artifact.id);
                if is_bug(artifact.artifact_type.as_deref()) {
                    feature_bugs += 1;
                    if covered {
                        feature_bugs_covered += 1;
                    }
                } else {
                    feature_use_cases += 1;
                    if covered {
                        feature_use_cases_covered += 1;
                    }
                }
            }

            total_use_cases += feature_use_cases;
            total_use_cases_covered += feature_use_cases_covered;
            total_bugs += feature_bugs;
            total_bugs_covered += feature_bugs_covered;

            json!({
                "id": feature.feature.id,
                "title": feature.feature.title,
                "createdAt": feature.feature.created_at,
                "updatedAt": feature.feature.updated_at.clone().unwrap_or_default(),
                "useCases": feature_use_cases,
                "useCasesCovered": feature_use_cases_covered,
                "bugs": feature_bugs,
                "bugsCovered": feature_bugs_covered,
            })
        })
        .collect();

    let lint_issues: Vec<Value> = lint_results
        .iter()
        .filter(|result| !result.is_valid)
        .map(|result| {
            let issue = result.issue.as_ref();
            json!({
                "file": result.file_path.display().to_string(),
                "message": issue.map_or_else(String::new, |value| value.message.clone()),
                "line": issue.and_then(|value| value.line),
                "column": issue.and_then(|value| value.column),
                "suggestion": issue.and_then(|value| value.suggestion.clone()),
            })
        })
        .collect();

    let valid_ucc_files = lint_results.iter().filter(|result| result.is_valid).count();
    let invalid_ucc_files = lint_results.len().saturating_sub(valid_ucc_files);

    json!({
        "summary": {
            "totalFeatures": features.len(),
            "totalUseCases": total_use_cases,
            "coveredUseCases": total_use_cases_covered,
            "useCaseCoveragePct": coverage_percentage(total_use_cases_covered, total_use_cases),
            "totalBugs": total_bugs,
            "coveredBugs": total_bugs_covered,
            "bugCoveragePct": coverage_percentage(total_bugs_covered, total_bugs),
            "validUccFiles": valid_ucc_files,
            "invalidUccFiles": invalid_ucc_files
        },
        "features": feature_rows,
        "lintIssues": lint_issues,
    })
}

fn is_bug(artifact_type: Option<&str>) -> bool {
    artifact_type.is_some_and(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("bug") || lower.contains("regression")
    })
}

const fn html_template() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>UseCaseCoverage Report</title>
    <link rel="stylesheet" href="./styles.css" />
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
  </head>
  <body>
    <header class="topbar">
      <h1>UseCaseCoverage Report</h1>
      <p>Generated from .ucc specifications and test coverage discovery</p>
    </header>

    <main class="container">
      <section class="metrics" id="metrics"></section>

      <section class="charts">
        <article class="card">
          <h2>Use Cases Coverage</h2>
          <canvas id="useCaseChart"></canvas>
        </article>
        <article class="card">
          <h2>Bugs Coverage</h2>
          <canvas id="bugChart"></canvas>
        </article>
      </section>

      <section class="card">
        <h2>Feature Breakdown</h2>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Feature ID</th>
                <th>Title</th>
                <th>Use Cases</th>
                <th>Covered</th>
                <th>Bugs</th>
                <th>Covered Bugs</th>
                <th>Created</th>
                <th>Updated</th>
              </tr>
            </thead>
            <tbody id="featureRows"></tbody>
          </table>
        </div>
      </section>

      <section class="card">
        <h2>Lint Results</h2>
        <ul id="lintList" class="lint-list"></ul>
      </section>
    </main>

    <script type="module" src="./app.js"></script>
  </body>
</html>
"#
}

const fn css_template() -> &'static str {
    r":root {
  --bg: #081421;
  --surface: #15202d;
  --surface-2: #1f2b38;
  --text: #d7e3f5;
  --muted: #9fb0c6;
  --primary: #a5c8ff;
  --accent: #f8be00;
  --ok: #66d9a6;
  --error: #ff8e8e;
  --border: #43474f;
}

* { box-sizing: border-box; }
body {
  margin: 0;
  font-family: Inter, system-ui, sans-serif;
  background: var(--bg);
  color: var(--text);
}

.topbar {
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
.topbar h1 { margin: 0; font-size: 1.4rem; }
.topbar p { margin: 0.35rem 0 0; color: var(--muted); }

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 1rem;
  display: grid;
  gap: 1rem;
}

.metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}
.metric {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 0.8rem;
}
.metric .label { color: var(--muted); font-size: 0.82rem; }
.metric .value { font-size: 1.4rem; font-weight: 700; margin-top: 0.25rem; }

.charts {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1rem;
}

.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 0.9rem;
}
.card h2 { margin: 0 0 0.65rem; font-size: 1.05rem; }

.table-wrap { overflow-x: auto; }
table {
  width: 100%;
  border-collapse: collapse;
  min-width: 760px;
}
th, td {
  text-align: left;
  padding: 0.55rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.9rem;
}
th { color: var(--muted); }

.lint-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.6rem;
}
.lint-item {
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 0.65rem;
  background: var(--surface-2);
}
.lint-item.ok { border-color: #2d654e; }
.lint-item.error { border-color: #6a3030; }
.lint-path { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.85rem; }
.lint-msg { margin-top: 0.35rem; color: var(--muted); font-size: 0.86rem; }

@media (min-width: 760px) {
  .metrics { grid-template-columns: repeat(4, minmax(0, 1fr)); }
}

@media (min-width: 1024px) {
  .charts { grid-template-columns: 1fr 1fr; }
}
"
}

#[allow(clippy::too_many_lines)]
const fn ts_template() -> &'static str {
    r#"type ReportData = {
  summary: {
    totalFeatures: number;
    totalUseCases: number;
    coveredUseCases: number;
    useCaseCoveragePct: number;
    totalBugs: number;
    coveredBugs: number;
    bugCoveragePct: number;
    validUccFiles: number;
    invalidUccFiles: number;
  };
  features: Array<{
    id: string;
    title: string;
    useCases: number;
    useCasesCovered: number;
    bugs: number;
    bugsCovered: number;
    createdAt: string;
    updatedAt: string;
  }>;
  lintIssues: Array<{
    file: string;
    message: string;
    line: number | null;
    column: number | null;
    suggestion: string | null;
  }>;
};

const metricKeys: Array<[string, keyof ReportData['summary']]> = [
  ['Features', 'totalFeatures'],
  ['Use Cases', 'totalUseCases'],
  ['Covered Use Cases', 'coveredUseCases'],
  ['Bugs', 'totalBugs'],
  ['Covered Bugs', 'coveredBugs'],
  ['Valid .ucc', 'validUccFiles'],
  ['Invalid .ucc', 'invalidUccFiles'],
];

async function loadData(): Promise<ReportData> {
  const response = await fetch('./data.json');
  return response.json();
}

function renderMetrics(data: ReportData): void {
  const root = document.getElementById('metrics');
  if (!root) return;

  root.innerHTML = metricKeys
    .map(([label, key]) => {
      const value = data.summary[key];
      return `<article class="metric"><div class="label">${label}</div><div class="value">${value}</div></article>`;
    })
    .join('');
}

function renderFeatureTable(data: ReportData): void {
  const table = document.getElementById('featureRows');
  if (!table) return;
  table.innerHTML = data.features
    .map(
      (feature) => `<tr>
        <td>${feature.id}</td>
        <td>${feature.title}</td>
        <td>${feature.useCases}</td>
        <td>${feature.useCasesCovered}</td>
        <td>${feature.bugs}</td>
        <td>${feature.bugsCovered}</td>
        <td>${feature.createdAt}</td>
        <td>${feature.updatedAt || '-'}</td>
      </tr>`
    )
    .join('');
}

function renderLint(data: ReportData): void {
  const list = document.getElementById('lintList');
  if (!list) return;

  if (data.lintIssues.length === 0) {
    list.innerHTML = `<li class="lint-item ok"><strong>All .ucc files passed lint validation.</strong></li>`;
    return;
  }

  list.innerHTML = data.lintIssues
    .map((issue) => {
      const where = issue.line ? `line ${issue.line}${issue.column ? `, col ${issue.column}` : ''}` : 'unknown location';
      return `<li class="lint-item error">
        <div class="lint-path">${issue.file}</div>
        <div class="lint-msg"><strong>${where}</strong>: ${issue.message}</div>
        ${issue.suggestion ? `<div class="lint-msg">Suggestion: ${issue.suggestion}</div>` : ''}
      </li>`;
    })
    .join('');
}

function renderCharts(data: ReportData): void {
  const useCaseCanvas = document.getElementById('useCaseChart') as HTMLCanvasElement | null;
  const bugCanvas = document.getElementById('bugChart') as HTMLCanvasElement | null;
  if (!useCaseCanvas || !bugCanvas) return;

  // @ts-ignore - Chart is provided by CDN at runtime.
  new Chart(useCaseCanvas, {
    type: 'doughnut',
    data: {
      labels: ['Covered', 'Missing'],
      datasets: [{
        data: [data.summary.coveredUseCases, Math.max(data.summary.totalUseCases - data.summary.coveredUseCases, 0)],
        backgroundColor: ['#a5c8ff', '#2a3643'],
      }],
    },
    options: { plugins: { legend: { labels: { color: '#d7e3f5' } } } },
  });

  // @ts-ignore - Chart is provided by CDN at runtime.
  new Chart(bugCanvas, {
    type: 'bar',
    data: {
      labels: data.features.map((feature) => feature.id),
      datasets: [
        { label: 'Bugs', data: data.features.map((feature) => feature.bugs), backgroundColor: '#f8be00' },
        { label: 'Covered', data: data.features.map((feature) => feature.bugsCovered), backgroundColor: '#a5c8ff' },
      ],
    },
    options: {
      responsive: true,
      plugins: { legend: { labels: { color: '#d7e3f5' } } },
      scales: {
        x: { ticks: { color: '#d7e3f5' }, grid: { color: '#2a3643' } },
        y: { ticks: { color: '#d7e3f5' }, grid: { color: '#2a3643' } },
      },
    },
  });
}

async function bootstrap(): Promise<void> {
  const data = await loadData();
  renderMetrics(data);
  renderFeatureTable(data);
  renderLint(data);
  renderCharts(data);
}

void bootstrap();
"#
}

const fn js_template() -> &'static str {
    // Intentionally mirrors app.ts so the report runs without a TS build step.
    r#"const metricKeys = [
  ['Features', 'totalFeatures'],
  ['Use Cases', 'totalUseCases'],
  ['Covered Use Cases', 'coveredUseCases'],
  ['Bugs', 'totalBugs'],
  ['Covered Bugs', 'coveredBugs'],
  ['Valid .ucc', 'validUccFiles'],
  ['Invalid .ucc', 'invalidUccFiles'],
];

async function loadData() {
  const response = await fetch('./data.json');
  return response.json();
}

function renderMetrics(data) {
  const root = document.getElementById('metrics');
  if (!root) return;
  root.innerHTML = metricKeys
    .map(([label, key]) => `<article class="metric"><div class="label">${label}</div><div class="value">${data.summary[key]}</div></article>`)
    .join('');
}

function renderFeatureTable(data) {
  const table = document.getElementById('featureRows');
  if (!table) return;
  table.innerHTML = data.features
    .map((feature) => `<tr>
      <td>${feature.id}</td>
      <td>${feature.title}</td>
      <td>${feature.useCases}</td>
      <td>${feature.useCasesCovered}</td>
      <td>${feature.bugs}</td>
      <td>${feature.bugsCovered}</td>
      <td>${feature.createdAt}</td>
      <td>${feature.updatedAt || '-'}</td>
    </tr>`)
    .join('');
}

function renderLint(data) {
  const list = document.getElementById('lintList');
  if (!list) return;
  if (data.lintIssues.length === 0) {
    list.innerHTML = `<li class="lint-item ok"><strong>All .ucc files passed lint validation.</strong></li>`;
    return;
  }
  list.innerHTML = data.lintIssues
    .map((issue) => {
      const where = issue.line ? `line ${issue.line}${issue.column ? `, col ${issue.column}` : ''}` : 'unknown location';
      return `<li class="lint-item error">
        <div class="lint-path">${issue.file}</div>
        <div class="lint-msg"><strong>${where}</strong>: ${issue.message}</div>
        ${issue.suggestion ? `<div class="lint-msg">Suggestion: ${issue.suggestion}</div>` : ''}
      </li>`;
    })
    .join('');
}

function renderCharts(data) {
  const useCaseCanvas = document.getElementById('useCaseChart');
  const bugCanvas = document.getElementById('bugChart');
  if (!useCaseCanvas || !bugCanvas) return;

  new Chart(useCaseCanvas, {
    type: 'doughnut',
    data: {
      labels: ['Covered', 'Missing'],
      datasets: [{
        data: [data.summary.coveredUseCases, Math.max(data.summary.totalUseCases - data.summary.coveredUseCases, 0)],
        backgroundColor: ['#a5c8ff', '#2a3643'],
      }],
    },
    options: { plugins: { legend: { labels: { color: '#d7e3f5' } } } },
  });

  new Chart(bugCanvas, {
    type: 'bar',
    data: {
      labels: data.features.map((feature) => feature.id),
      datasets: [
        { label: 'Bugs', data: data.features.map((feature) => feature.bugs), backgroundColor: '#f8be00' },
        { label: 'Covered', data: data.features.map((feature) => feature.bugsCovered), backgroundColor: '#a5c8ff' },
      ],
    },
    options: {
      responsive: true,
      plugins: { legend: { labels: { color: '#d7e3f5' } } },
      scales: {
        x: { ticks: { color: '#d7e3f5' }, grid: { color: '#2a3643' } },
        y: { ticks: { color: '#d7e3f5' }, grid: { color: '#2a3643' } },
      },
    },
  });
}

async function bootstrap() {
  const data = await loadData();
  renderMetrics(data);
  renderFeatureTable(data);
  renderLint(data);
  renderCharts(data);
}

void bootstrap();
"#
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;
    use use_case_coverage_core::domain::{
        Artifact, ArtifactCoverageIndex, FeatureDocument, FeatureMetadata, Priority, UccLintIssue,
        UccLintResult,
    };

    use super::generate_html_report;

    #[test]
    fn generates_report_bundle_into_dot_ucc_folder() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        let features = vec![FeatureDocument {
            source_path: root.join("feature.ucc"),
            schema_version: "1.0".to_string(),
            feature: FeatureMetadata {
                id: "feat-1".to_string(),
                title: "Feature One".to_string(),
                created_at: "2026-05-10".to_string(),
                updated_at: None,
                description: "desc".to_string(),
            },
            tags: vec![],
            platforms: vec![],
            related_features: vec![],
            artifacts: vec![Artifact {
                id: "ucc-feat-1".to_string(),
                artifact_type: None,
                created_at: "2026-05-10".to_string(),
                title: "Use case".to_string(),
                priority: Priority::High,
                related: vec![],
                steps: vec![],
                expected: vec![],
            }],
        }];

        let lint_results = vec![UccLintResult {
            file_path: root.join("feature.ucc"),
            is_valid: true,
            issue: None,
        }];

        let coverage_index = ArtifactCoverageIndex::default();

        generate_html_report(root, &features, &lint_results, &coverage_index)
            .expect("report should be generated");

        let output = root.join(".ucc");
        assert!(output.join("index.html").exists());
        assert!(output.join("styles.css").exists());
        assert!(output.join("app.ts").exists());
        assert!(output.join("app.js").exists());
        assert!(output.join("data.json").exists());

        let html = fs::read_to_string(output.join("index.html")).expect("html should be readable");
        assert!(html.contains("UseCaseCoverage Report"));

        let json = fs::read_to_string(output.join("data.json")).expect("json should be readable");
        assert!(json.contains("\"totalFeatures\": 1"));
    }

    #[test]
    fn includes_lint_errors_in_output_data() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        let lint_results = vec![UccLintResult {
            file_path: root.join("broken.ucc"),
            is_valid: false,
            issue: Some(UccLintIssue {
                message: "invalid type".to_string(),
                line: Some(12),
                column: Some(8),
                suggestion: Some("Fix schema type".to_string()),
            }),
        }];

        generate_html_report(root, &[], &lint_results, &ArtifactCoverageIndex::default())
            .expect("report should be generated");

        let json =
            fs::read_to_string(root.join(".ucc/data.json")).expect("json should be readable");
        assert!(json.contains("broken.ucc"));
        assert!(json.contains("Fix schema type"));
    }
}
