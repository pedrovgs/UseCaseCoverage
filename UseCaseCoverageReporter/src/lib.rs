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

/// Generates a responsive report bundle (`HTML + CSS + TS + JS`) in `output_dir`.
///
/// # Errors
///
/// Returns an error if report directory or files cannot be written.
pub fn generate_html_report(
    output_dir: &Path,
    repo_name: &str,
    features: &[FeatureDocument],
    lint_results: &[UccLintResult],
    coverage_index: &ArtifactCoverageIndex,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(output_dir)?;

    let report_data = build_report_data(features, lint_results, coverage_index);
    let report_json = serde_json::to_string_pretty(&report_data)
        .map_err(|error| std::io::Error::other(format!("JSON serialization failed: {error}")))?;

    let report_date = chrono::Local::now().format("%b %d, %Y").to_string();
    fs::write(output_dir.join("index.html"), html_template(repo_name, &report_date, &report_json))?;
    fs::write(output_dir.join("styles.css"), css_template())?;
    fs::write(output_dir.join("app.ts"), ts_template())?;
    fs::write(output_dir.join("app.js"), js_template())?;
    fs::write(output_dir.join("data.json"), &report_json)?;

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
        "growth": build_growth_data(features, coverage_index),
    })
}

fn build_growth_data(
    features: &[FeatureDocument],
    coverage_index: &ArtifactCoverageIndex,
) -> Value {
    fn is_leap(y: i32) -> bool {
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
    }

    fn current_year_month() -> (i32, u32) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut days = secs / 86400;

        let mut y = 1970i32;
        loop {
            let days_in_year = if is_leap(y) { 366 } else { 365 };
            if days < days_in_year {
                break;
            }
            days -= days_in_year;
            y += 1;
        }

        let month_days = if is_leap(y) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut m = 1u32;
        for &md in month_days.iter() {
            if days < md {
                break;
            }
            days -= md;
            m += 1;
        }

        (y, m)
    }

    fn parse_year_month(date_str: &str) -> Option<(i32, u32)> {
        let parts: Vec<&str> = date_str.split('-').collect();
        let year = parts.first()?.parse::<i32>().ok()?;
        let month = parts.get(1)?.parse::<u32>().ok()?;
        if (1..=12).contains(&month) {
            Some((year, month))
        } else {
            None
        }
    }

    fn month_short_name(month: u32) -> &'static str {
        match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "??",
        }
    }

    let (now_year, now_month) = current_year_month();

    let mut start_year = now_year;
    let mut start_month = now_month;
    for _ in 0..11 {
        if start_month == 1 {
            start_month = 12;
            start_year -= 1;
        } else {
            start_month -= 1;
        }
    }

    let mut months = Vec::with_capacity(12);
    let mut features_count = vec![0u32; 12];
    let mut use_cases_count = vec![0u32; 12];
    let mut bugs_count = vec![0u32; 12];
    let mut artifacts_count = vec![0u32; 12];
    let mut covered_use_cases_count = vec![0u32; 12];
    let mut covered_bugs_count = vec![0u32; 12];

    let mut y = start_year;
    let mut m = start_month;
    for idx in 0..12 {
        months.push(month_short_name(m).to_string());

        for feature in features {
            if let Some((fy, fm)) = parse_year_month(&feature.feature.created_at) {
                if fy == y && fm == m {
                    features_count[idx] += 1;
                }
            }
            for artifact in &feature.artifacts {
                if let Some((ay, am)) = parse_year_month(&artifact.created_at) {
                    if ay == y && am == m {
                        artifacts_count[idx] += 1;
                        let covered = coverage_index.is_covered(&artifact.id);
                        if is_bug(artifact.artifact_type.as_deref()) {
                            bugs_count[idx] += 1;
                            if covered {
                                covered_bugs_count[idx] += 1;
                            }
                        } else {
                            use_cases_count[idx] += 1;
                            if covered {
                                covered_use_cases_count[idx] += 1;
                            }
                        }
                    }
                }
            }
        }

        if m == 12 {
            m = 1;
            y += 1;
        } else {
            m += 1;
        }
    }

    json!({
        "months": months,
        "features": features_count,
        "useCases": use_cases_count,
        "bugs": bugs_count,
        "artifacts": artifacts_count,
        "coveredUseCases": covered_use_cases_count,
        "coveredBugs": covered_bugs_count,
    })
}

fn is_bug(artifact_type: Option<&str>) -> bool {
    artifact_type.is_some_and(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("bug") || lower.contains("regression")
    })
}

fn html_template(repo_name: &str, report_date: &str, data_json: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Analysis Report</title>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=Roboto:wght@400;500;600;700&display=swap" rel="stylesheet" />
    <link rel="icon" type="image/svg+xml" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'%3E%3Crect x='4' y='4' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='36' y='4' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='4' y='36' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='36' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='36' y='68' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='68' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='4' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3Crect x='36' y='36' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3Crect x='4' y='68' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3C/svg%3E" />
    <link rel="stylesheet" href="./styles.css" />
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
  </head>
  <body>
    <script id="report-data" type="application/json">{data_json}</script>

    <div class="layout">
      <aside class="sidebar">
        <div class="logo-box">
          <svg width="64" height="64" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
            <rect x="4" y="4" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="36" y="4" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="4" y="36" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="68" y="36" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="36" y="68" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="68" y="68" width="28" height="28" rx="6" fill="#1e3656" />
            <rect x="68" y="4" width="28" height="28" rx="8" fill="#fcb714" />
            <rect x="36" y="36" width="28" height="28" rx="8" fill="#fcb714" />
            <rect x="4" y="68" width="28" height="28" rx="8" fill="#fcb714" />            
          </svg>
        </div>
        <nav>
          <a href="#" class="nav-item active">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>
            Dashboard
          </a>
          <a href="#" class="nav-item">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>
            Inventory
          </a>
        </nav>
      </aside>

      <main class="main-content">
        <header class="topbar">
          <div class="topbar-left">
            <h1>Analysis Report</h1>
            <span class="repo-name">{repo_name}</span>
            <span class="report-date">{report_date}</span>
          </div>

        </header>

        <div class="container">
          <section class="metrics" id="metrics"></section>

          <section class="charts-row">
            <article class="card">
              <div class="card-header">
                <div>
                  <h2>Use Cases Growth</h2>
                  <span class="subtitle">Created per month</span>
                </div>
              </div>
              <div class="chart-container"><canvas id="useCaseGrowthChart"></canvas></div>
            </article>
            <article class="card">
              <div class="card-header">
                <div>
                  <h2>Features Growth</h2>
                  <span class="subtitle">Created per month</span>
                </div>
              </div>
              <div class="chart-container"><canvas id="featureGrowthChart"></canvas></div>
            </article>
            <article class="card">
              <div class="card-header">
                <div>
                  <h2>Bugs Growth</h2>
                  <span class="subtitle">Created per month</span>
                </div>
              </div>
              <div class="chart-container"><canvas id="bugGrowthChart"></canvas></div>
            </article>
          </section>

          <section class="card large-chart-card">
            <div class="card-header">
              <div>
                <h2>Feature Coverage Progress</h2>
                <span class="subtitle">Artifacts vs Covered over time</span>
              </div>
            </div>
            <div class="large-chart-container"><canvas id="featureCoverageChart"></canvas></div>
          </section>

          <section class="card">
            <div class="card-header"><h2>Feature Breakdown</h2></div>
            <div class="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th data-sort="title" class="sortable">Feature Title <span class="sort-icon"></span></th>
                    <th data-sort="useCases" class="sortable">Use Cases <span class="sort-icon"></span></th>
                    <th data-sort="useCasesCovered" class="sortable">UC Covered <span class="sort-icon"></span></th>
                    <th data-sort="ucPct" class="sortable">UC % <span class="sort-icon"></span></th>
                    <th data-sort="bugs" class="sortable">Bugs <span class="sort-icon"></span></th>
                    <th data-sort="bugsCovered" class="sortable">Bugs Covered <span class="sort-icon"></span></th>
                    <th data-sort="bugsPct" class="sortable">Bugs % <span class="sort-icon"></span></th>
                    <th data-sort="updatedAt" class="sortable">Updated At <span class="sort-icon"></span></th>
                  </tr>
                </thead>
                <tbody id="featureRows"></tbody>
              </table>
            </div>
          </section>
        </div>
      </main>
    </div>
    <script src="./app.js"></script>
  </body>
</html>"##
    )
}

const fn css_template() -> &'static str {
    r##":root {
  --bg-main: #0b1118;
  --bg-sidebar: #080c12;
  --bg-card: #151b23;
  --border: #242d38;
  --text-main: #ffffff;
  --text-muted: #8b9eb0;
  --text-blue: #96afc9;
  --accent: #fcb714;
}

body {
  margin: 0;
  font-family: 'Roboto', sans-serif;
  background: var(--bg-main);
  color: var(--text-main);
}

.layout {
  display: flex;
  min-height: 100vh;
}

.sidebar {
  width: 250px;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
}

.logo-box {
  padding: 2rem;
  display: flex;
  justify-content: center;
}

.sidebar nav {
  display: flex;
  flex-direction: column;
  padding: 0 1rem;
  gap: 0.5rem;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-radius: 6px;
  color: var(--text-muted);
  text-decoration: none;
  font-weight: 500;
  font-size: 0.95rem;
  transition: all 0.2s;
}

.nav-item.active {
  background: var(--accent);
  color: #000;
}
.nav-item svg { width: 18px; height: 18px; }

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem 2rem;
  background: var(--bg-main);
  border-bottom: 1px solid var(--border);
}

.topbar-left {
  display: flex;
  align-items: center;
  gap: 1.5rem;
}
.topbar h1 { margin: 0; font-size: 1.25rem; font-weight: 600; }
.repo-name { color: var(--text-blue); font-size: 0.85rem; text-decoration: underline; text-underline-offset: 4px; }
.report-date { color: var(--text-muted); font-size: 0.85rem; }

.date-picker {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-muted);
  padding: 0.4rem 0.75rem;
  border-radius: 4px;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.85rem;
  cursor: pointer;
}

.container {
  padding: 2rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.metrics {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 1rem;
}

.metric {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 1.25rem 1.5rem;
  position: relative;
  display: flex;
  flex-direction: column;
}

.metric.accent-border {
  border-right: 3px solid var(--accent);
}

.metric .label {
  color: var(--text-muted);
  font-size: 0.65rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-weight: 600;
  margin-bottom: 0.5rem;
}
.metric .value-row {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}
.metric .value {
  font-size: 2rem;
  font-weight: 400;
  color: var(--text-blue);
}
.metric .subtitle {
  font-size: 0.75rem;
  font-weight: 600;
}
.metric .subtitle.yellow { color: var(--accent); }
.metric .subtitle.gray { color: var(--text-muted); font-weight: 400; }
.metric .value-row.accent-value .value { color: var(--accent); }

.charts-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
}

.card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
}

.card-header {
  padding: 1.25rem 1.25rem 0.5rem;
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}
.card-header h2 { margin: 0; font-size: 0.95rem; font-weight: 500; color: #fff;}
.card-header .subtitle { display: block; margin-top: 0.25rem; font-size: 0.75rem; color: var(--text-muted); }

.chart-container {
  padding: 0 1.25rem 1.25rem;
  position: relative;
  height: 250px;
}

.large-chart-container {
  padding: 0 1.25rem 1.25rem;
  position: relative;
  height: 350px;
}

.table-wrap { overflow-x: auto; padding: 0 1.25rem 1.25rem; }
table {
  width: 100%;
  border-collapse: collapse;
}
th, td {
  text-align: left;
  padding: 1rem 0;
  border-bottom: 1px solid var(--border);
  font-size: 0.85rem;
}
th { 
  color: var(--text-main); 
  font-weight: 600; 
  padding-bottom: 0.5rem; 
  border-bottom-width: 2px;
}
td { color: var(--text-blue); }

.lint-list { list-style: none; padding: 0 1.25rem 1.25rem; margin: 0; display: grid; gap: 0.5rem; }
.lint-item { padding: 0.75rem; border: 1px solid var(--border); border-radius: 4px; background: rgba(255,255,255,0.02);}
.lint-path { font-family: monospace; color: var(--text-blue); margin-bottom: 0.25rem; font-size: 0.85rem;}
.lint-msg { font-size: 0.85rem; color: var(--text-muted); }

th.sortable { cursor: pointer; user-select: none; white-space: nowrap; transition: color 0.2s; }
th.sortable:hover { color: #fff; background: rgba(255,255,255,0.05); }
.sort-icon { display: inline-block; width: 12px; margin-left: 6px; font-style: normal; opacity: 0.3; }
.sort-icon::after { content: '↕'; }
th.sort-asc, th.sort-desc { color: var(--accent); }
th.sort-asc .sort-icon { opacity: 1; }
th.sort-asc .sort-icon::after { content: '↑'; }
th.sort-desc .sort-icon { opacity: 1; }
th.sort-desc .sort-icon::after { content: '↓'; }
.lint-msg { font-size: 0.85rem; color: var(--text-muted); }
"##
}

#[allow(clippy::too_many_lines)]
const fn ts_template() -> &'static str {
    r##"// @ts-nocheck

function loadData() {
  const el = document.getElementById('report-data');
  return JSON.parse(el.textContent);
}

function renderMetrics(data) {
  const root = document.getElementById('metrics');
  if (!root) return;
  root.innerHTML = `
    <article class="metric">
      <div class="label">Total Features</div>
      <div class="value-row">
        <div class="value">${data.summary.totalFeatures}</div>
        <div class="subtitle yellow">+2 this week</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Use Cases</div>
      <div class="value-row">
        <div class="value">${data.summary.totalUseCases}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalUseCases / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border">
      <div class="label">Covered Cases</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredUseCases}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.useCaseCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Bugs</div>
      <div class="value-row">
        <div class="value">${data.summary.totalBugs}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalBugs / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border" style="border-right:0;">
      <div class="label">Covered Bugs</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredBugs}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.bugCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
  `;
}

let _sortKey = 'title';
let _sortAsc = true;

function getSortValue(feature, key) {
  switch (key) {
    case 'title': return feature.title.toLowerCase();
    case 'useCases': return feature.useCases;
    case 'useCasesCovered': return feature.useCasesCovered;
    case 'ucPct': return feature.useCases > 0 ? feature.useCasesCovered / feature.useCases : -1;
    case 'bugs': return feature.bugs;
    case 'bugsCovered': return feature.bugsCovered;
    case 'bugsPct': return feature.bugs > 0 ? feature.bugsCovered / feature.bugs : -1;
    case 'updatedAt': return feature.updatedAt || feature.createdAt;
    default: return '';
  }
}

function renderFeatureTable(data) {
  const tbody = document.getElementById('featureRows');
  if (!tbody) return;

  const sorted = [...data.features].sort((a, b) => {
    const va = getSortValue(a, _sortKey);
    const vb = getSortValue(b, _sortKey);
    if (va < vb) return _sortAsc ? -1 : 1;
    if (va > vb) return _sortAsc ? 1 : -1;
    return 0;
  });

  tbody.innerHTML = sorted
    .map(
      (feature) => `<tr>
        <td style="color:#a5c8ff; max-width:200px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" title="${feature.title}">${feature.title}</td>
        <td>${feature.useCases}</td>
        <td>${feature.useCasesCovered}</td>
        <td>${feature.useCases > 0 ? ((feature.useCasesCovered / feature.useCases) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.bugs}</td>
        <td>${feature.bugsCovered}</td>
        <td>${feature.bugs > 0 ? ((feature.bugsCovered / feature.bugs) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.updatedAt ? feature.updatedAt : feature.createdAt}</td>
      </tr>`
    )
    .join('');

  // Update header sort indicators
  document.querySelectorAll('th.sortable').forEach((th) => {
    th.classList.remove('sort-asc', 'sort-desc');
    if (th.dataset.sort === _sortKey) {
      th.classList.add(_sortAsc ? 'sort-asc' : 'sort-desc');
    }
  });
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
  Chart.defaults.color = '#8b9eb0';
  Chart.defaults.font.family = 'Roboto';

  const chartConfig = {
    plugins: { legend: { display: false } },
    scales: {
      x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
      y: { display: false, grid: { display: false } }
    },
    maintainAspectRatio: false
  };

  const months = data.growth.months;

  const useCaseCanvas = document.getElementById('useCaseGrowthChart');
  if (useCaseCanvas) {
    new Chart(useCaseCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    });
  }

  const featureCanvas = document.getElementById('featureGrowthChart');
  if (featureCanvas) {
    new Chart(featureCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Features', data: data.growth.features, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 }
        ]
      },
      options: chartConfig
    });
  }

  const bugCanvas = document.getElementById('bugGrowthChart');
  if (bugCanvas) {
    new Chart(bugCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Bugs', data: data.growth.bugs, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    });
  }

  const progressCanvas = document.getElementById('featureCoverageChart');
  if (progressCanvas) {
    new Chart(progressCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 },
          { label: 'Covered UC', data: data.growth.coveredUseCases, borderColor: '#a5c8ff', borderWidth: 2, pointBackgroundColor: '#a5c8ff', tension: 0.1, borderDash: [4,3] },
          { label: 'Bugs', data: data.growth.bugs, borderColor: '#fcb714', borderWidth: 2, pointBackgroundColor: '#fcb714', tension: 0.1 },
          { label: 'Covered Bugs', data: data.growth.coveredBugs, borderColor: '#e5a410', borderWidth: 2, pointBackgroundColor: '#e5a410', tension: 0.1, borderDash: [4,3] },
        ]
      },
      options: {
        plugins: { 
          legend: { display: true, position: 'top', align: 'end', labels: { boxWidth: 12, color: '#8b9eb0' } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
          y: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
        },
        maintainAspectRatio: false
      }
    });
  }
}

function bootstrap() {
  const data = loadData();
  renderMetrics(data);
  renderFeatureTable(data);
  renderLint(data);
  renderCharts(data);

  document.querySelectorAll('th.sortable').forEach((th) => {
    th.addEventListener('click', () => {
      const key = th.dataset.sort;
      if (_sortKey === key) {
        _sortAsc = !_sortAsc;
      } else {
        _sortKey = key;
        _sortAsc = true;
      }
      renderFeatureTable(data);
    });
  });
}
"##
}

const fn js_template() -> &'static str {
    r##"
function loadData() {
  const el = document.getElementById('report-data');
  return JSON.parse(el.textContent);
}

function renderMetrics(data) {
  const root = document.getElementById('metrics');
  if (!root) return;
  root.innerHTML = `
    <article class="metric">
      <div class="label">Total Features</div>
      <div class="value-row">
        <div class="value">${data.summary.totalFeatures}</div>
        <div class="subtitle yellow">+2 this week</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Use Cases</div>
      <div class="value-row">
        <div class="value">${data.summary.totalUseCases}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalUseCases / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border">
      <div class="label">Covered Cases</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredUseCases}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.useCaseCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Bugs</div>
      <div class="value-row">
        <div class="value">${data.summary.totalBugs}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalBugs / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border" style="border-right:0;">
      <div class="label">Covered Bugs</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredBugs}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.bugCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
  `;
}

let _sortKey = 'title';
let _sortAsc = true;

function getSortValue(feature, key) {
  switch (key) {
    case 'title': return feature.title.toLowerCase();
    case 'useCases': return feature.useCases;
    case 'useCasesCovered': return feature.useCasesCovered;
    case 'ucPct': return feature.useCases > 0 ? feature.useCasesCovered / feature.useCases : -1;
    case 'bugs': return feature.bugs;
    case 'bugsCovered': return feature.bugsCovered;
    case 'bugsPct': return feature.bugs > 0 ? feature.bugsCovered / feature.bugs : -1;
    case 'updatedAt': return feature.updatedAt || feature.createdAt;
    default: return '';
  }
}

function renderFeatureTable(data) {
  const tbody = document.getElementById('featureRows');
  if (!tbody) return;

  const sorted = [...data.features].sort((a, b) => {
    const va = getSortValue(a, _sortKey);
    const vb = getSortValue(b, _sortKey);
    if (va < vb) return _sortAsc ? -1 : 1;
    if (va > vb) return _sortAsc ? 1 : -1;
    return 0;
  });

  tbody.innerHTML = sorted
    .map(
      (feature) => `<tr>
        <td style="color:#a5c8ff; max-width:200px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" title="${feature.title}">${feature.title}</td>
        <td>${feature.useCases}</td>
        <td>${feature.useCasesCovered}</td>
        <td>${feature.useCases > 0 ? ((feature.useCasesCovered / feature.useCases) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.bugs}</td>
        <td>${feature.bugsCovered}</td>
        <td>${feature.bugs > 0 ? ((feature.bugsCovered / feature.bugs) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.updatedAt ? feature.updatedAt : feature.createdAt}</td>
      </tr>`
    )
    .join('');

  // Update header sort indicators
  document.querySelectorAll('th.sortable').forEach((th) => {
    th.classList.remove('sort-asc', 'sort-desc');
    if (th.dataset.sort === _sortKey) {
      th.classList.add(_sortAsc ? 'sort-asc' : 'sort-desc');
    }
  });
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
  Chart.defaults.color = '#8b9eb0';
  Chart.defaults.font.family = 'Roboto';

  const chartConfig = {
    plugins: { legend: { display: false } },
    scales: {
      x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
      y: { display: false, grid: { display: false } }
    },
    maintainAspectRatio: false
  };

  const months = data.growth.months;

  const useCaseCanvas = document.getElementById('useCaseGrowthChart');
  if (useCaseCanvas) {
    new Chart(useCaseCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    });
  }

  const featureCanvas = document.getElementById('featureGrowthChart');
  if (featureCanvas) {
    new Chart(featureCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Features', data: data.growth.features, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 }
        ]
      },
      options: chartConfig
    });
  }

  const bugCanvas = document.getElementById('bugGrowthChart');
  if (bugCanvas) {
    new Chart(bugCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Bugs', data: data.growth.bugs, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    });
  }

  const progressCanvas = document.getElementById('featureCoverageChart');
  if (progressCanvas) {
    new Chart(progressCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 },
          { label: 'Covered UC', data: data.growth.coveredUseCases, borderColor: '#a5c8ff', borderWidth: 2, pointBackgroundColor: '#a5c8ff', tension: 0.1, borderDash: [4,3] },
          { label: 'Bugs', data: data.growth.bugs, borderColor: '#fcb714', borderWidth: 2, pointBackgroundColor: '#fcb714', tension: 0.1 },
          { label: 'Covered Bugs', data: data.growth.coveredBugs, borderColor: '#e5a410', borderWidth: 2, pointBackgroundColor: '#e5a410', tension: 0.1, borderDash: [4,3] },
        ]
      },
      options: {
        plugins: { 
          legend: { display: true, position: 'top', align: 'end', labels: { boxWidth: 12, color: '#8b9eb0' } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
          y: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
        },
        maintainAspectRatio: false
      }
    });
  }
}

function bootstrap() {
  const data = loadData();
  renderMetrics(data);
  renderFeatureTable(data);
  renderLint(data);
  renderCharts(data);

  document.querySelectorAll('th.sortable').forEach((th) => {
    th.addEventListener('click', () => {
      const key = th.dataset.sort;
      if (_sortKey === key) {
        _sortAsc = !_sortAsc;
      } else {
        _sortKey = key;
        _sortAsc = true;
      }
      renderFeatureTable(data);
    });
  });
}

void bootstrap();
"##
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

        let output_dir = root.join(".ucc");
        generate_html_report(&output_dir, "test-repo", &features, &lint_results, &coverage_index)
            .expect("report should be generated");

        assert!(output_dir.join("index.html").exists());
        assert!(output_dir.join("styles.css").exists());
        assert!(output_dir.join("app.ts").exists());
        assert!(output_dir.join("app.js").exists());
        assert!(output_dir.join("data.json").exists());

        let html =
            fs::read_to_string(output_dir.join("index.html")).expect("html should be readable");
        assert!(html.contains("Analysis Report"));

        let json =
            fs::read_to_string(output_dir.join("data.json")).expect("json should be readable");
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

        let output_dir = root.join(".ucc");
        generate_html_report(
            &output_dir,
            "test-repo",
            &[],
            &lint_results,
            &ArtifactCoverageIndex::default(),
        )
        .expect("report should be generated");

        let json =
            fs::read_to_string(output_dir.join("data.json")).expect("json should be readable");
        assert!(json.contains("broken.ucc"));
        assert!(json.contains("Fix schema type"));
    }
}
