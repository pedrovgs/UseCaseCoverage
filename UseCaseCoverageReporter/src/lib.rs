#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use chrono::{Datelike, Local, NaiveDate};
use serde_json::{json, Value};
use use_case_coverage_core::coverage_percentage;
use use_case_coverage_core::domain::{ArtifactCoverageIndex, FeatureDocument, UccLintResult};

const D3_JS: &str = include_str!("../assets/vendor/d3.v7.min.js");
const D3_CLOUD_JS: &str = include_str!("../assets/vendor/d3.layout.cloud.js");
const CHART_JS: &str = include_str!("../assets/vendor/chart.js");
const FLEXSEARCH_JS: &str = include_str!("../assets/vendor/flexsearch.bundle.js");

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
    let vendor_dir = output_dir.join("vendor");
    fs::create_dir_all(&vendor_dir)?;

    let report_data = build_report_data(features, lint_results, coverage_index);
    let report_json = serde_json::to_string_pretty(&report_data)
        .map_err(|error| std::io::Error::other(format!("JSON serialization failed: {error}")))?;

    let date_str = chrono::Local::now().format("%b %d, %Y").to_string();
    fs::write(output_dir.join("index.html"), html_template(repo_name, &date_str, &report_json))?;
    fs::write(output_dir.join("styles.css"), css_template())?;
    fs::write(output_dir.join("app.ts"), ts_template())?;
    fs::write(output_dir.join("app.js"), js_template())?;
    fs::write(output_dir.join("data.json"), &report_json)?;

    fs::write(vendor_dir.join("d3.v7.min.js"), D3_JS)?;
    fs::write(vendor_dir.join("d3.layout.cloud.js"), D3_CLOUD_JS)?;
    fs::write(vendor_dir.join("chart.js"), CHART_JS)?;
    fs::write(vendor_dir.join("flexsearch.bundle.js"), FLEXSEARCH_JS)?;

    Ok(())
}

fn is_bug(artifact_type: Option<&str>) -> bool {
    artifact_type.is_some_and(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("bug") || lower.contains("regression")
    })
}

fn current_year_month() -> (i32, u32) {
    let now = Local::now();
    (now.year(), now.month())
}

fn parse_year_month(date_str: &str) -> Option<(i32, u32)> {
    // Try full date first (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Some((date.year(), date.month()));
    }
    // Fallback to YYYY-MM if only that is provided
    let parts: Vec<&str> = date_str.split('-').collect();
    let year = parts.first()?.parse::<i32>().ok()?;
    let month = parts.get(1)?.parse::<u32>().ok()?;
    if (1..=12).contains(&month) {
        Some((year, month))
    } else {
        None
    }
}

const fn month_short_name(month: u32) -> &'static str {
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
                "description": feature.feature.description,
                "createdAt": feature.feature.created_at,
                "updatedAt": feature.feature.updated_at.clone().unwrap_or_default(),
                "useCases": feature_use_cases,
                "useCasesCovered": feature_use_cases_covered,
                "bugs": feature_bugs,
                "bugsCovered": feature_bugs_covered,
                "platforms": feature.platforms,
                "artifacts": feature.artifacts.iter().map(|a| {
                    json!({
                        "id": a.id,
                        "title": a.title,
                        "createdAt": a.created_at,
                        "updatedAt": a.updated_at,
                        "type": a.artifact_type.clone().unwrap_or_else(|| "usecase".to_string()),
                        "priority": format!("{:?}", a.priority),
                        "isCovered": coverage_index.is_covered(&a.id),
                        "coverageLocations": coverage_index.for_artifact(&a.id).iter().map(|loc| {
                            json!({
                                "path": loc.file_path.to_string_lossy(),
                                "line": loc.line,
                            })
                        }).collect::<Vec<_>>(),
                        "steps": a.steps,
                        "expected": a.expected,
                        "platforms": a.platforms,
                        "tags": a.tags,
                        "coverageGapReason": a.coverage_gap_reason,
                    })
                }).collect::<Vec<_>>(),
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

#[allow(clippy::too_many_lines)]
fn build_growth_data(
    features: &[FeatureDocument],
    coverage_index: &ArtifactCoverageIndex,
) -> Value {
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

#[allow(clippy::too_many_lines)]
fn html_template(repo_name: &str, report_date: &str, data_json: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>UCC Report - {report_date}</title>
    <link rel="icon" type="image/svg+xml" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'%3E%3Crect x='4' y='4' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='36' y='4' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='4' y='36' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='36' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='36' y='68' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='68' width='28' height='28' rx='6' fill='%231e3656'/%3E%3Crect x='68' y='4' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3Crect x='36' y='36' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3Crect x='4' y='68' width='28' height='28' rx='8' fill='%23fcb714'/%3E%3C/svg%3E" />
    <link rel="stylesheet" href="./styles.css" />
    <script src="./vendor/chart.js"></script>
    <script src="./vendor/flexsearch.bundle.js"></script>
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
        <nav id="sidebarNav">
          <a href="#dashboard" class="nav-item active">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>
            Dashboard
          </a>
          <a href="#inventory" class="nav-item">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>
                        <span class="label">Inventory</span>
          </a>
          <a href="#tags" class="nav-item">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path><line x1="7" y1="7" x2="7.01" y2="7"></line></svg>
            <span class="label">Tags</span>
          </a>
          <a href="#gaps" class="nav-item">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>
            <span class="label">Gaps</span>
          </a>
        </nav>
        <div class="sidebar-footer">
          <a href="https://github.com/sponsors/pedrovgs" target="_blank" rel="noopener noreferrer">
            Crafted with ❤️ by Pedro Gómez
          </a>
        </div>
      </aside>

      <main class="main-content">
        <header class="topbar">
          <div class="topbar-left">
            <h1>UCC Report</h1>
            <span class="repo-name">{repo_name}</span>
            <span class="report-date">{report_date}</span>
          </div>

        </header>

        <div id="dashboardView" class="container">
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

          <div id="featureDetailView" class="container" style="display:none;"></div>

          <div id="inventoryView" class="container" style="display:none;">
            <div class="detail-header" style="padding:0; margin-bottom: 2rem;">
              <button class="back-btn" onclick="navigate(event, '')">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
                Back to Dashboard
              </button>
            </div>
            <section class="charts-row" style="grid-template-columns: repeat(2, 1fr); gap: 1.5rem; margin-bottom: 2rem;">
              <article class="card">
                <div class="card-header" style="text-align:center;"><h2>Use Cases Distribution</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="invUCChart"></canvas></div>
              </article>
              <article class="card">
                <div class="card-header" style="text-align:center;"><h2>Bugs Distribution</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="invBugChart"></canvas></div>
              </article>
              <article class="card">
                <div class="card-header" style="text-align:center;"><h2>Covered Use Cases</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="invCoveredUCChart"></canvas></div>
              </article>
              <article class="card">
                <div class="card-header" style="text-align:center;"><h2>Covered Bugs</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="invCoveredBugChart"></canvas></div>
              </article>
            </section>

            <section class="card">
              <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <h2>Features inventory <span id="featCount" style="font-size:0.9rem; color:var(--accent); margin-left:10px; font-weight:normal;"></span></h2>
                <input type="text" id="invSearch" placeholder="Search title, desc, use cases..." 
                       style="background:rgba(255,255,255,0.05); border:1px solid var(--border); color:#fff; padding:8px 16px; border-radius:6px; width:350px; outline:none; transition: border-color 0.2s;">
              </div>
              <div class="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th style="width:250px;">Title</th>
                      <th>Description</th>
                      <th style="width:120px;">Last Updated</th>
                      <th style="width:150px;">Coverage</th>
                    </tr>
                  </thead>
                  <tbody id="invRows"></tbody>
                </table>
              </div>
            </section>
          </div>

          <div id="tagsView" class="container" style="display:none;">
            <div class="detail-header" style="padding:0; margin-bottom: 2rem;">
              <button class="back-btn" onclick="navigate(event, '')">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
                Back to Dashboard
              </button>
            </div>
            
            <div style="display:grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-bottom: 2rem;">
              <section class="card">
                <div class="card-header"><h2>Platforms Distribution</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="platformsChart"></canvas></div>
              </section>
              <section class="card">
                <div class="card-header"><h2>Tags Distribution</h2></div>
                <div class="chart-container" style="height: 300px;"><canvas id="tagsChart"></canvas></div>
              </section>
            </div>

            <div style="display:grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;">
              <section class="card">
                <div class="card-header"><h2>Platforms Inventory</h2></div>
                <div class="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Platform</th>
                        <th>Occurrences</th>
                      </tr>
                    </thead>
                    <tbody id="platformsTableBody"></tbody>
                  </table>
                </div>
              </section>
              <section class="card">
                <div class="card-header"><h2>Tags Inventory</h2></div>
                <div class="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Tag</th>
                        <th>Occurrences</th>
                      </tr>
                    </thead>
                    <tbody id="tagsTableBody"></tbody>
                  </table>
                </div>
              </section>
            </div>
          </div>

          <div id="gapsView" class="container" style="display:none;">
            <div class="detail-header" style="padding:0; margin-bottom: 2rem;">
              <button class="back-btn" onclick="navigate(event, '')">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
                Back to Dashboard
              </button>
            </div>

            <div style="display:grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-bottom: 2rem;">
              <section class="card">
                <div class="card-header">
                   <h2>Coverage gaps</h2>
                    <span class="subtitle">Full descriptions of coverage gap reasons</span>
                </div>
                <div id="gapSentenceCloud" style="height: 400px; display:flex; justify-content:center; align-items:center;"></div>
              </section>
              <section class="card">
                <div class="card-header">
                   <h2>Common Gap Themes</h2>
                    <span class="subtitle">Tokenized words from coverage gap reasons</span>
                </div>
                <div id="gapWordCloud" style="height: 400px; display:flex; justify-content:center; align-items:center;"></div>
              </section>
            </div>

            <div style="display:grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-bottom: 2rem;">
              <section class="card">
                <div class="card-header"><h2>Top Coverage Needs</h2></div>
                <div class="table-wrap">
                  <table style="font-size:0.85rem;">
                    <thead>
                      <tr>
                        <th>Feature</th>
                        <th>Missing UC coverage</th>
                        <th>Missing Bug coverage</th>
                      </tr>
                    </thead>
                    <tbody id="topMissingTableBody"></tbody>
                  </table>
                </div>
              </section>
               <section class="card">
                <div class="card-header"><h2>Most Gaps Declared</h2></div>
                <div class="table-wrap">
                  <table style="font-size:0.85rem;">
                    <thead>
                      <tr>
                        <th>Feature</th>
                        <th>Gap Explanations</th>
                      </tr>
                    </thead>
                    <tbody id="topGapsTableBody"></tbody>
                  </table>
                </div>
              </section>
            </div>

            <div style="display:grid; grid-template-columns: 1fr; gap: 1.5rem;">
              <section class="card">
                <div class="card-header"><h2>Coverage Gaps Inventory</h2></div>
                <div class="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Reason</th>
                        <th>Count</th>
                        <th>Impacted Features</th>
                      </tr>
                    </thead>
                    <tbody id="gapsInventoryTableBody"></tbody>
                  </table>
                </div>
              </section>
            </div>
          </div>
        </div>
        <footer class="report-footer">
          💌 All the feedback is welcome! I want to maintain the tool free from any type of tracking so it's completely anonymous and your data only belongs to you. If you are using the app or if you have any feedback for me, please let me know at <a href="mailto:pedrovicente.gomez@gmail.com">pedrovicente.gomez@gmail.com</a>. Thanks! ✨
        </footer>
      </main>
    </div>
    <script src="./vendor/d3.v7.min.js"></script>
    <script src="./vendor/d3.layout.cloud.js"></script>
    <script src="./app.js"></script>
  </body>
</html>"##
    )
}

#[allow(clippy::too_many_lines)]
const fn css_template() -> &'static str {
    r":root {
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
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji';
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

.sidebar-footer {
  padding: 1.5rem;
  border-top: 1px solid var(--border);
  text-align: center;
}

.sidebar-footer a {
  color: var(--text-muted);
  text-decoration: none;
  font-size: 0.75rem;
  transition: color 0.2s;
}

.sidebar-footer a:hover {
  color: var(--accent);
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
  margin-bottom: 0.75rem;
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

#dashboardView > section {
  margin-bottom: 0.5rem;
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

tbody tr { cursor: pointer; transition: background 0.1s; }
tbody tr:hover { background: rgba(255,255,255,0.02); }

.detail-header { margin-bottom: 2rem; position: relative; }
.back-btn { background: transparent; border: 1px solid var(--border); color: var(--text-blue); padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; margin-bottom: 1rem; display: flex; align-items: center; gap: 0.5rem; }
.back-btn:hover { background: var(--border); color: #fff; }
.detail-title { font-size: 2rem; margin: 0 0 0.5rem 0; }
.detail-meta { color: var(--text-muted); font-size: 0.9rem; display: flex; gap: 1.5rem; }
.detail-desc { margin: 1.5rem 0; line-height: 1.6; color: var(--text-blue); max-width: 800px; }

.artifact-grid { display: grid; grid-template-columns: 1fr; gap: 1rem; }
.artifact-card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 8px; padding: 1.25rem; position: relative; min-height: 100px; }
.artifact-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.75rem; }
.artifact-title { font-weight: 600; font-size: 1.1rem; color: #fff; }
.badge { padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; text-transform: uppercase; }
.badge-bug { background: rgba(239, 68, 68, 0.1); color: #ef4444; border: 1px solid rgba(239, 68, 68, 0.2); }
.badge-usecase { background: rgba(59, 130, 246, 0.1); color: #3b82f6; border: 1px solid rgba(59, 130, 246, 0.2); }
.badge-covered { background: rgba(16, 185, 129, 0.1); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.2); }
.badge-missing { background: rgba(245, 158, 11, 0.1); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.2); }
.badge-critical { background: rgba(239, 68, 68, 0.2); color: #ff4d4d; border: 1px solid #ff4d4d; animation: pulse 2s infinite; }
@keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.7; } 100% { opacity: 1; } }

.artifact-card.critical-missing { border: 1px solid rgba(239, 68, 68, 0.5); background: rgba(239, 68, 68, 0.05); }

.artifact-body { font-size: 0.9rem; color: var(--text-blue); }
.steps-list { margin: 0.5rem 0 0 1.25rem; padding: 0; }
.steps-list li { margin-bottom: 0.25rem; }
.expected-section { margin-top: 0.75rem; border-top: 1px solid var(--border); padding-top: 0.75rem; }
.report-footer {
  margin-top: auto;
  padding: 2rem;
  border-top: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 0.85rem;
  text-align: center;
  line-height: 1.6;
}
.report-footer a {
  color: var(--text-blue);
  text-decoration: underline;
  text-underline-offset: 4px;
}
.report-footer a:hover {
  color: var(--accent);
}
"
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
let _detailFilter = 'all';
let _detailSort = 'priority';
let _detailUCChart = null;
let _detailBugChart = null;

const PRIORITY_MAP = { 'Highest': 5, 'High': 4, 'Medium': 3, 'Low': 2, 'None': 1 };

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


function renderPlatformIcon(platform) {
  const p = platform.toLowerCase();
  if (p.includes('apple') || p.includes('ios') || p.includes('mac') || p.includes('iphone')) return '🍎';
  if (p.includes('android')) return '🤖';
  if (p.includes('windows')) return '🪟';
  if (p.includes('web') || p.includes('browser')) return '🌐';
  return '📱';
}
function renderPriorityIcon(priority) {
  const p = priority.toLowerCase();
  let color = '#8b9eb0';
  let svg = '';

  if (p === 'highest') {
    color = '#ff4d4d';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12M12 11L5 18M12 11L19 18" stroke-width="2.5" />';
  } else if (p === 'high') {
    color = '#ff8533';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12" stroke-width="2" />';
  } else if (p === 'medium') {
    color = '#fcb714';
    svg = '<path d="M5 12H19" stroke-width="3" />';
  } else if (p === 'low') {
    color = '#3b82f6';
    svg = '<path d="M12 5V19M12 19L5 12M12 19L19 12" stroke-width="2" />';
  } else {
    svg = '<circle cx="12" cy="12" r="4" fill="currentColor" />';
  }

  return `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="${color}" style="vertical-align: middle; margin-right: 4px;">${svg}</svg>`;
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
      (feature) => `<tr onclick="navigate(event, '#feature/${feature.id}')" style="cursor:pointer;">
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

function renderFeatureDetail(data, featureId) {
  const container = document.getElementById('featureDetailView');
  const feature = data.features.find(f => f.id === featureId);
  if (!feature) {
    container.innerHTML = '<h1>Feature not found</h1>';
    return;
  }

  let artifacts = [...feature.artifacts];

  // Search
  if (_detailSearchText) {
    const s = _detailSearchText.toLowerCase();
    artifacts = artifacts.filter(a => 
      a.title.toLowerCase().includes(s) || 
      (a.steps || []).some(step => step.toLowerCase().includes(s)) ||
      (a.expected || []).some(exp => exp.toLowerCase().includes(s)) ||
      (a.platforms || []).some(p => p.toLowerCase().includes(s))    );
  }

  // Filtering
  if (_detailFilter === 'covered') artifacts = artifacts.filter(a => a.isCovered);
  if (_detailFilter === 'missing') artifacts = artifacts.filter(a => !a.isCovered);
  if (_detailFilter === 'critical') artifacts = artifacts.filter(a => !a.isCovered && (a.priority === 'High' || a.priority === 'Highest'));

  // Sorting
  artifacts.sort((a, b) => {
    let va, vb;
    switch (_detailSort) {
      case 'priority':
        va = PRIORITY_MAP[a.priority] || 0;
        vb = PRIORITY_MAP[b.priority] || 0;
        return vb - va; // Default High to Low
      case 'createdAt':
        va = a.createdAt || '';
        vb = b.createdAt || '';
        break;
      case 'updatedAt':
        va = a.updatedAt || a.createdAt || '';
        vb = b.updatedAt || b.createdAt || '';
        break;
      case 'status':
        va = a.isCovered ? 1 : 0;
        vb = b.isCovered ? 1 : 0;
        break;
      default: return 0;
    }
    if (va < vb) return 1;
    if (va > vb) return -1;
    return 0;
  });

  container.innerHTML = `
    <div class="detail-header">
      <button class="back-btn" onclick="navigate(event, '')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
        Back to Dashboard
      </button>
      <h1 class="detail-title">${feature.title}</h1>
      <div class="detail-meta">
        <span>ID: <strong>${feature.id}</strong></span>
        <span>Created: <strong>${feature.createdAt}</strong></span>
        ${feature.updatedAt ? `<span>Updated: <strong>${feature.updatedAt}</strong></span>` : ''}
      </div>
      <div class="detail-desc">${feature.description || 'No description provided.'}</div>
    </div>

    <section class="charts-row" style="margin-bottom: 2rem; grid-template-columns: 1fr 1fr;">
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Use Cases Coverage</h2>
             <span class="subtitle">${feature.useCasesCovered} / ${feature.useCases} (${feature.useCases > 0 ? ((feature.useCasesCovered/feature.useCases)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailUCChart"></canvas></div>
      </article>
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Bugs Coverage</h2>
             <span class="subtitle">${feature.bugsCovered} / ${feature.bugs} (${feature.bugs > 0 ? ((feature.bugsCovered/feature.bugs)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailBugChart"></canvas></div>
      </article>
    </section>

    <div class="detail-controls card" style="display:grid; grid-template-columns: 1fr auto; grid-template-rows: auto auto; padding:1.25rem; gap:1rem; margin-bottom: 2rem; border-radius:12px; background:var(--bg-card); border:1px solid var(--border);">
       <!-- Column 1: Search & Results -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.01); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.75rem;"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
         <input type="text" id="detailSearch" value="${_detailSearchText}" placeholder="Search artifacts by title, steps, or content..." 
                oninput="_detailSearchText=this.value; renderFeatureDetail(window._lastData, '${featureId}')"
                style="background:transparent; border:none; color:#fff; padding:1rem 0; width:100%; outline:none; font-size:0.9rem;">
       </div>
       <!-- TOP RIGHT: Results -->
       <div style="grid-column: 2; display:flex; align-items:center; padding:0 1rem; color:var(--text-muted); font-size:0.8rem; white-space:nowrap; border-bottom:1px solid transparent;">
         <strong>${artifacts.length}</strong> &nbsp;results
       </div>
       <!-- BOTTOM LEFT: Filter -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"></polygon></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Filter</span>
         <select id="filterSelect" onchange="_detailFilter=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="all" ${_detailFilter==='all'?'selected':''}>All</option>
           <option value="covered" ${_detailFilter==='covered'?'selected':''}>Covered</option>
           <option value="missing" ${_detailFilter==='missing'?'selected':''}>Missing</option>
           <option value="critical" ${_detailFilter==='critical'?'selected':''}>Critical</option>
         </select>
       </div>
       <!-- BOTTOM RIGHT: Sort -->
       <div style="grid-column: 2; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><path d="M3 12h18M3 6h18M3 18h18"></path></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Sort</span>
         <select id="sortSelect" onchange="_detailSort=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="priority" ${_detailSort==='priority'?'selected':''}>Priority</option>
           <option value="createdAt" ${_detailSort==='createdAt'?'selected':''}>Created</option>
           <option value="updatedAt" ${_detailSort==='updatedAt'?'selected':''}>Updated</option>
           <option value="status" ${_detailSort==='status'?'selected':''}>Coverage</option>
         </select>
       </div>
    </div>

    <div class="artifact-section">
      <div class="artifact-grid">
        ${artifacts.map(a => {
          const isHigh = a.priority === 'High' || a.priority === 'Highest';
          const isCritical = isHigh && !a.isCovered;
          return `
          <div class="artifact-card ${isCritical ? 'critical-missing' : ''}">
            <div class="artifact-head">
              <div style="display:flex; align-items:center; gap:0.75rem">
                <span class="badge ${a.type.toLowerCase().includes('bug') ? 'badge-bug' : 'badge-usecase'}">${a.type}</span>
                <span class="artifact-title">${a.title}</span>
              </div>
              <div style="display:flex; gap:0.5rem">
                ${isCritical ? '<span class="badge badge-critical">CRITICAL</span>' : ''}
                <span class="badge ${a.isCovered ? 'badge-covered' : 'badge-missing'}">${a.isCovered ? 'Covered' : 'Missing'}</span>
              </div>
            </div>
            <div class="artifact-body">
              <div style="display:flex; gap:1.5rem; margin-bottom: 0.5rem; font-size:0.8rem; color:var(--text-muted); align-items:center;">
                <div style="display:flex; align-items:center;">ID: <strong style="color:#fff; margin-left:0.4rem; margin-right:1rem;">${a.id}</strong></div>
                <div style="display:flex; align-items:center;">Priority: ${renderPriorityIcon(a.priority)} <strong style="color:#fff">${a.priority}</strong></div>
                ${a.platforms && a.platforms.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Platforms:</span>
                    ${a.platforms.map(p => `<div style="display:inline-flex; align-items:center; margin-right:0.6rem; font-size:0.7rem;">${renderPlatformIcon(p)} <span style="color:#fff; text-transform:uppercase; letter-spacing:0.02em;">${p}</span></div>`).join('')}
                  </div>
                ` : ''}
                ${a.tags && a.tags.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Tags:</span>
                    ${a.tags.map(t => `<span class="badge" style="background:rgba(252,183,20,0.1); color:#fcb714; border:1px solid rgba(252,183,20,0.2); font-size:0.65rem; text-transform:uppercase; padding: 2px 6px;">${t}</span>`).join('')}
                  </div>
                ` : ''}
                <div>Created: <strong>${a.createdAt}</strong></div>
                ${a.updatedAt ? `<div>Updated: <strong>${a.updatedAt}</strong></div>` : ''}
              </div>
              ${a.isCovered && a.coverageLocations && a.coverageLocations.length > 0 ? `
                <div style="position: absolute; bottom: 1.25rem; right: 1.25rem; display:flex; flex-wrap:wrap; gap:0.5rem; justify-content: flex-end;">
                  ${a.coverageLocations.map(loc => `
                    <a href="vscode://file/${loc.path}:${loc.line}" class="back-btn" style="margin:0; padding:4px 8px; font-size:0.75rem; text-decoration:none; display:inline-flex; align-items:center; gap:0.4rem; background:rgba(0,122,204,0.1); border-color:rgba(0,122,204,0.3); color:#4fc1ff;">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M23.15 2.58L19.8 1.45c-.32-.11-.66.1-.66.44v5.45c0 .12-.05.23-.14.31L13 13.7l-3.3-3.04c-.16-.14-.4-.14-.56 0L1 17.72c-.12.11-.12.3 0 .41l3.3 3.04c.16.14.4.14.56 0l1.24-1.14 7.22-6.66c.09-.08.14-.19.14-.31V6.26l6.68-6.16c.16-.14.4-.14.56 0l2.45 2.26c.12.11.12.3 0 .41L18.46 9c-.16.15-.16.4 0 .55l4.69 4.31c.12.11.12.3 0 .41l-2.45 2.26c-.16.14-.4.14-.56 0L13 12.23V17.74c0 .34.34.55.66.44l3.35-1.13c.12-.04.23-.04.35 0l2.45.83c.32.11.66-.1.66-.44V3.02c0-.12-.05-.23-.14-.31l-.18-.13z"/></svg>
                      ${loc.path.split('/').pop().split('\\').pop()}:L${loc.line}
                    </a>
                  `).join('')}
                </div>
              ` : ''}
              ${a.steps && a.steps.length ? `
                <div style="font-weight:600; color:var(--text-muted); font-size:0.8rem; margin-top:0.75rem; text-transform:uppercase;">Steps</div>
                <ol class="steps-list">
                  ${a.steps.map(s => `<li>${s}</li>`).join('')}
                </ol>
              ` : ''}
              ${a.expected && a.expected.length ? `
                <div class="expected-section">
                  <div class="expected-title">Expected Result</div>
                  <ul class="steps-list" style="list-style-type: disc">
                    ${a.expected.map(e => `<li>${e}</li>`).join('')}
                  </ul>
                </div>
              ` : ''}
              ${!a.isCovered && a.coverageGapReason ? `
                <div style="margin-top:1rem; padding:0.75rem; background:rgba(252,183,20,0.05); border-left:3px solid var(--accent); border-radius:4px;">
                  <div style="font-weight:600; color:var(--accent); font-size:0.75rem; text-transform:uppercase; margin-bottom:0.25rem;">Coverage Gap Reason</div>
                  <div style="font-size:0.85rem; color:var(--text-blue); font-style:italic;">"${a.coverageGapReason}"</div>
                </div>
              ` : ''}
            </div>
          </div>
        `;
      }).join('')}
        ${artifacts.length === 0 ? '<div class="card" style="padding:2rem; text-align:center; color:var(--text-muted)">No artifacts match the selected criteria.</div>' : ''}
      </div>
    </div>
  `;

  renderDetailCharts(feature);

  const searchInput = document.getElementById('detailSearch');
  if (searchInput) {
    searchInput.focus();
    searchInput.setSelectionRange(searchInput.value.length, searchInput.value.length);
  }
}

function renderDetailCharts(feature) {
  if (_detailUCChart) _detailUCChart.destroy();
  if (_detailBugChart) _detailBugChart.destroy();

  const animationConfig = {
    duration: 1200,
    easing: 'easeOutElastic',
    delay: (context) => context.dataIndex * 300
  };

  const ctxUC = document.getElementById('detailUCChart');
  if (ctxUC) {
    _detailUCChart = new Chart(ctxUC, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.useCasesCovered, feature.useCases - feature.useCasesCovered],
          backgroundColor: ['#10b981', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }

  const ctxBug = document.getElementById('detailBugChart');
  if (ctxBug) {
    _detailBugChart = new Chart(ctxBug, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.bugsCovered, feature.bugs - feature.bugsCovered],
          backgroundColor: ['#ef4444', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }
}

let _detailFilter = 'all';
let _detailSort = 'priority';
let _detailSearchText = '';
let _detailUCChart = null;
let _detailBugChart = null;
let _dashboardCharts = [];
let _invCharts = [];
let _tagsViewCharts = [];
let _searchIndex = null;

const PRIORITY_MAP = { 'Highest': 5, 'High': 4, 'Medium': 3, 'Low': 2, 'None': 1 };

function navigate(e, hash) {
  if (e.metaKey || e.ctrlKey) {
    window.open(window.location.pathname + hash, '_blank');
  } else {
    window.location.hash = hash;
  }
}

function renderGapsView(data) {
  const topMissingTable = document.getElementById('topMissingTableBody');
  const topGapsTable = document.getElementById('topGapsTableBody');
  const gapsInventoryTable = document.getElementById('gapsInventoryTableBody');
  if (!topMissingTable || !topGapsTable || !gapsInventoryTable) return;

  const gapReasons = {};
  const featureGaps = [];
  const missingCoverage = [];

  data.features.forEach(f => {
    let gapsInFeature = 0;
    const missingTests = f.useCases - f.useCasesCovered;
    const missingBugs = f.bugs - f.bugsCovered;

    f.artifacts.forEach(a => {
      if (a.coverageGapReason) {
        gapsInFeature++;
        const reason = a.coverageGapReason;
        gapReasons[reason] = gapReasons[reason] || { count: 0, features: new Set() };
        gapReasons[reason].count++;
        gapReasons[reason].features.add(f.title);
      }
    });

    if (gapsInFeature > 0) {
      featureGaps.push({ id: f.id, title: f.title, count: gapsInFeature });
    }
    if (missingTests > 0 || missingBugs > 0) {
      missingCoverage.push({ id: f.id, title: f.title, missingTests, missingBugs });
    }
  });

  // Top Missing
  missingCoverage.sort((a, b) => (b.missingTests + b.missingBugs) - (a.missingTests + a.missingBugs));
  topMissingTable.innerHTML = missingCoverage.slice(0, 10).map(m => `
    <tr onclick="navigate(event, '#feature/${m.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${m.title}</td>
      <td style="color:#fcb714; font-weight:bold;">${m.missingTests}</td>
      <td style="color:#ef4444; font-weight:bold;">${m.missingBugs}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No missing coverage</td></tr>';

  // Top Gaps
  featureGaps.sort((a, b) => b.count - a.count);
  topGapsTable.innerHTML = featureGaps.slice(0, 10).map(g => `
    <tr onclick="navigate(event, '#feature/${g.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${g.title}</td>
      <td style="font-weight:bold;">${g.count} artifacts</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No gaps declared</td></tr>';

  // Gaps Inventory
  const reasons = Object.entries(gapReasons).map(([reason, d]) => ({
    reason,
    count: d.count,
    features: Array.from(d.features).join(', ')
  })).sort((a, b) => b.count - a.count);

  gapsInventoryTable.innerHTML = reasons.map(r => `
    <tr>
      <td style="color:#fcb714; max-width:300px;">${r.reason}</td>
      <td style="font-weight:bold;">${r.count}</td>
      <td style="font-size:0.75rem; color:var(--text-muted);">${r.features}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No gap reasons found</td></tr>';

  if (typeof d3 !== 'undefined') {
    renderGapCloud(reasons);
  }
}

function renderGapCloud(reasons) {
  const container = document.getElementById('gapCloud');
  if (!container) return;
  container.innerHTML = '';

  const width = container.offsetWidth || 600;
  const height = 400;

  const words = reasons.map(r => ({ text: r.reason, size: 10 + (Math.min(r.count, 20) * 10) }));

  const layout = d3.layout.cloud()
    .size([width, height])
    .words(words)
    .padding(5)
    .rotate(() => (~~(Math.random() * 2) * 90))
    .font("Impact")
    .fontSize(d => d.size)
    .on("end", (words) => {
      const svg = d3.select("#gapCloud").append("svg")
        .attr("width", layout.size()[0])
        .attr("height", layout.size()[1])
        .append("g")
        .attr("transform", "translate(" + layout.size()[0] / 2 + "," + layout.size()[1] / 2 + ")");

      const texts = svg.selectAll("text")
        .data(words)
        .enter().append("text")
        .style("font-size", "0px")
        .style("font-family", "Impact")
        .style("fill", () => d3.schemeTableau10[Math.floor(Math.random() * 10)])
        .attr("text-anchor", "middle")
        .attr("transform", d => "translate(" + [d.x, d.y] + ")rotate(" + d.rotate + ")")
        .text(d => d.text);

      texts.transition()
        .duration(1000)
        .style("font-size", d => d.size + "px");
    });

  layout.start();
}

function handleRouting(data) {
  window.scrollTo(0, 0);
  const hash = window.location.hash || '#dashboard';
  const dashboard = document.getElementById('dashboardView');
  const detail = document.getElementById('featureDetailView');
  const inventory = document.getElementById('inventoryView');
  const tags = document.getElementById('tagsView');
  const gaps = document.getElementById('gapsView');
  
  [dashboard, detail, inventory, tags, gaps].forEach(v => { if(v) v.style.display = 'none'; });
  
  document.querySelectorAll('.nav-item').forEach(link => {
    const href = link.getAttribute('href');
    link.classList.toggle('active', href === hash || (hash === '#dashboard' && href === '#dashboard'));
  });

  if (hash.startsWith('#feature/')) {
    detail.style.display = 'block';
    renderFeatureDetail(data, hash.replace('#feature/', ''));
  } else if (hash === '#inventory') {
    inventory.style.display = 'block';
    renderInventory(data);
  } else if (hash === '#tags') {
    if (tags) tags.style.display = 'block';
    renderTagsView(data);
  } else if (hash === '#gaps') {
    if (gaps) gaps.style.display = 'block';
    renderGapsView(data);
  } else {
    dashboard.style.display = 'block';
    renderCharts(data);
  }
}

function renderTagsView(data) {
  const tagTable = document.getElementById('tagsTableBody');
  const platformTable = document.getElementById('platformsTableBody');
  if (!tagTable || !platformTable) return;

  const tagCounts = {};
  const platformCounts = {};

  data.features.forEach(f => {
    (f.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    (f.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
    (f.artifacts || []).forEach(a => {
        (a.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
        (a.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    });
  });

  const tags = Object.entries(tagCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);
  const platforms = Object.entries(platformCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);

  tagTable.innerHTML = tags.map(item => `
    <tr>
      <td style="color:#fcb714;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No tags found</td></tr>';

  platformTable.innerHTML = platforms.map(item => `
    <tr>
      <td style="color:#a5c8ff;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No platforms found</td></tr>';

  renderTagsViewCharts(tags, platforms);
}

function renderTagsViewCharts(tags, platforms) {
  _tagsViewCharts.forEach(c => c.destroy());
  _tagsViewCharts = [];

  const createChart = (id, items, color) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _tagsViewCharts.push(new Chart(ctx, {
      type: 'bar',
      data: {
        labels: items.map(i => i.name),
        datasets: [{
          data: items.map(i => i.count),
          backgroundColor: color,
          borderRadius: 4
        }]
      },
      options: {
        indexAxis: 'y',
        animation: { duration: 1000, easing: 'easeOutQuart' },
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
          y: { grid: { display: false }, ticks: { color: '#fff' } }
        },
        maintainAspectRatio: false
      }
    }));
  };

  createChart('platformsChart', platforms, '#a5c8ff');
  createChart('tagsChart', tags, '#fcb714');
}

function renderInventory(data) {
  if (!_searchIndex && typeof FlexSearch !== 'undefined') {
    _searchIndex = new FlexSearch.Document({
      document: {
        id: "id",
        index: ["title", "description", "content"],
        store: ["id"]
      },
      tokenize: "forward"
    });
    
    data.features.forEach(f => {
       const content = (f.artifacts || []).map(a => 
         `${a.title} ${(a.steps || []).join(' ')} ${(a.expected || []).join(' ')} ${(a.tags || []).join(' ')}`
       ).join(' ');
       const tagContent = (f.tags || []).join(' ');
       const platformContent = [...(f.platforms || []), ...(f.artifacts || []).flatMap(a => a.platforms || [])].join(' ');
       _searchIndex.add({
          id: f.id,
          title: f.title,
          description: f.description || "",
          content: `${content} ${tagContent} ${platformContent}`
       });
    });
  }

  renderInventoryCharts(data);
  renderInventoryTable(data);
  
  const search = document.getElementById('invSearch');
  if (search) {
    search.oninput = (e) => renderInventoryTable(data, e.target.value);
  }
}

function renderInventoryCharts(data) {
  _invCharts.forEach(c => c.destroy());
  _invCharts = [];
  
  const features = data.features;
  const labels = features.map(f => f.title);
  const colors = features.map((_, i) => `hsl(${(i * 360 / features.length) % 360}, 65%, 50%)`);

  const animationConfig = {
    duration: 200,
    easing: 'easeOutQuart',
    delay: (context) => context.dataIndex * 20
  };

  const createChart = (id, label, values, total) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _invCharts.push(new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: labels,
        datasets: [{ data: values, backgroundColor: colors, borderWidth: 0 }]
      },
      options: {
        animation: animationConfig,
        plugins: { 
          legend: { 
            display: true, 
            position: 'right', 
            labels: { color: '#8b9eb0', boxWidth: 12, padding: 10, font: { size: 10 } } 
          },
          tooltip: { callbacks: { label: (ctx) => `${ctx.label}: ${ctx.raw} ${label}` } }
        },
        maintainAspectRatio: false,
        cutout: '65%'
      },
      plugins: [{
        id: 'centerText',
        beforeDraw: (chart) => {
          const { ctx, width, height } = chart;
          ctx.save();
          ctx.font = 'bold 1.2rem Roboto';
          ctx.fillStyle = '#fff';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const centerLeft = chart.chartArea.left + (chart.chartArea.right - chart.chartArea.left) / 2;
          ctx.fillText(total, centerLeft, height / 2 - 10);
          ctx.font = '0.7rem Roboto';
          ctx.fillStyle = '#8b9eb0';
          ctx.fillText('TOTAL', centerLeft, height / 2 + 15);
          ctx.restore();
        }
      }]
    }));
  };

  const totalUC = features.reduce((sum, f) => sum + f.useCases, 0);
  const totalBugs = features.reduce((sum, f) => sum + f.bugs, 0);
  const totalCovUC = features.reduce((sum, f) => sum + f.useCasesCovered, 0);
  const totalCovBugs = features.reduce((sum, f) => sum + f.bugsCovered, 0);

  createChart('invUCChart', 'Use Cases', features.map(f => f.useCases), totalUC);
  createChart('invBugChart', 'Bugs', features.map(f => f.bugs), totalBugs);
  createChart('invCoveredUCChart', 'Covered UC', features.map(f => f.useCasesCovered), totalCovUC);
  createChart('invCoveredBugChart', 'Covered Bugs', features.map(f => f.bugsCovered), totalCovBugs);
}

function renderInventoryTable(data, filter = '') {
  const tbody = document.getElementById('invRows');
  if (!tbody) return;

  let matched = data.features;
  if (filter && _searchIndex) {
    const results = _searchIndex.search(filter);
    const ids = new Set();
    results.forEach(r => r.result.forEach(id => ids.add(id)));
    matched = data.features.filter(feat => ids.has(feat.id));
  }

  const countEl = document.getElementById('featCount');
  if (countEl) countEl.textContent = `(${matched.length} features)`;

  tbody.innerHTML = matched.map(feat => `
    <tr onclick="navigate(event, '#feature/${feat.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${feat.title}</td>
      <td style="color:var(--text-muted); font-size:0.85rem;">${feat.description || '-'}</td>
      <td style="color:var(--text-muted); font-size:0.8rem;">${feat.lastModifiedAt || feat.updatedAt || feat.createdAt}</td>
      <td>
        <div style="font-size:0.8rem;">UC: <strong>${feat.useCasesCovered}/${feat.useCases}</strong></div>
        <div style="font-size:0.8rem;">Bugs: <strong>${feat.bugsCovered}/${feat.bugs}</strong></div>
      </td>
    </tr>
  `).join('');
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
  _dashboardCharts.forEach(c => c.destroy());
  _dashboardCharts = [];

  let delayed = false;
  const animationConfig = {
    onComplete: () => { delayed = true; },
    delay: (context) => {
      let delay = 0;
      if (context.type === 'data' && context.mode === 'default' && !delayed) {
        delay = context.dataIndex * 300 + context.datasetIndex * 100;
      }
      return delay;
    }
  };

  const chartConfig = {
    animation: animationConfig,
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
    _dashboardCharts.push(new Chart(useCaseCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const featureCanvas = document.getElementById('featureGrowthChart');
  if (featureCanvas) {
    _dashboardCharts.push(new Chart(featureCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Features', data: data.growth.features, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 }
        ]
      },
      options: chartConfig
    }));
  }

  const bugCanvas = document.getElementById('bugGrowthChart');
  if (bugCanvas) {
    _dashboardCharts.push(new Chart(bugCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Bugs', data: data.growth.bugs, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const progressCanvas = document.getElementById('featureCoverageChart');
  if (progressCanvas) {
    _dashboardCharts.push(new Chart(progressCanvas, {
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
        animation: animationConfig,
        plugins: { 
          legend: { display: true, position: 'top', align: 'end', labels: { boxWidth: 12, color: '#8b9eb0' } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
          y: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
        },
        maintainAspectRatio: false
      }
    }));
  }
}

function bootstrap() {
  const data = loadData();
  window._lastData = data;
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

  window.addEventListener('hashchange', () => handleRouting(data));
  handleRouting(data);
}

void bootstrap();
"##
}

#[allow(clippy::too_many_lines)]
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
let _detailFilter = 'all';
let _detailSort = 'priority';
let _detailSearchText = '';
let _detailUCChart = null;
let _detailBugChart = null;
let _dashboardCharts = [];
let _invCharts = [];
let _tagsViewCharts = [];
let _searchIndex = null;

const PRIORITY_MAP = { 'Highest': 5, 'High': 4, 'Medium': 3, 'Low': 2, 'None': 1 };

function navigate(e, hash) {
  if (e.metaKey || e.ctrlKey) {
    window.open(window.location.pathname + hash, '_blank');
  } else {
    window.location.hash = hash;
  }
}

function renderGapsView(data) {
  const topMissingTable = document.getElementById('topMissingTableBody');
  const topGapsTable = document.getElementById('topGapsTableBody');
  const gapsInventoryTable = document.getElementById('gapsInventoryTableBody');
  if (!topMissingTable || !topGapsTable || !gapsInventoryTable) return;

  const gapReasons = {};
  const featureGaps = [];
  const missingCoverage = [];
  
  const stopWords = new Set([
    "a", "an", "the", "and", "or", "but", "if", "then", "else", "when", "at", "from", "by", "for", "with", 
    "about", "against", "between", "into", "through", "during", "before", "after", "above", "below", "to", 
    "from", "up", "down", "in", "out", "on", "off", "over", "under", "again", "further", "then", "once", 
    "here", "there", "when", "where", "why", "how", "all", "any", "both", "each", "few", "more", "most", 
    "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "s", 
    "t", "can", "will", "just", "don", "should", "now", "is", "are", "was", "were", "be", "been", "being", 
    "have", "has", "had", "do", "does", "did", "am", "me", "my", "myself", "we", "our", "ours", "ourselves", 
    "you", "your", "yours", "yourself", "yourselves", "he", "him", "his", "himself", "she", "her", "hers", 
    "herself", "it", "its", "itself", "they", "them", "their", "theirs", "themselves", "what", "which", 
    "who", "whom", "this", "that", "these", "those", "doing", "would", "could", "should", "ought", "might", 
    "must", "shall", "cannot", "couldn't", "didn't", "doesn't", "hadn't", "hasn't", "haven't", "isn't", 
    "mustn't", "shan't", "shouldn't", "wasn't", "weren't", "won't", "wouldn't", "i"
  ].map(w => w.toLowerCase()));

  const cloudWordCounts = {};
  const cloudSentenceCounts = {};
  const processText = (text) => {
    if (!text) return;
    text.toLowerCase().split(/\W+/).forEach(word => {
      if (word.length > 2 && !stopWords.has(word.toLowerCase())) {
        cloudWordCounts[word] = (cloudWordCounts[word] || 0) + 1;
      }
    });
  };
  const processSentence = (text) => {
    if (!text) return;
    const lower = text.toLowerCase().trim();
    if (lower.length > 0) {
      cloudSentenceCounts[lower] = (cloudSentenceCounts[lower] || 0) + 1;
    }
  };

  data.features.forEach(f => {
    let gapsInFeature = 0;
    const missingTests = f.useCases - f.useCasesCovered;
    const missingBugs = f.bugs - f.bugsCovered;

    f.artifacts.forEach(a => {
      if (!a.isCovered) {
        if (a.coverageGapReason) {
          processText(a.coverageGapReason);
          processSentence(a.coverageGapReason);
          gapsInFeature++;
          const reason = a.coverageGapReason;
          gapReasons[reason] = gapReasons[reason] || { count: 0, features: new Set() };
          gapReasons[reason].count++;
          gapReasons[reason].features.add(f.title);
        }
      }
    });

    if (gapsInFeature > 0) {
      featureGaps.push({ id: f.id, title: f.title, count: gapsInFeature });
    }
    if (missingTests > 0 || missingBugs > 0) {
      missingCoverage.push({ id: f.id, title: f.title, missingTests, missingBugs });
    }
  });

  // Top Missing
  missingCoverage.sort((a, b) => (b.missingTests + b.missingBugs) - (a.missingTests + a.missingBugs));
  topMissingTable.innerHTML = missingCoverage.slice(0, 10).map(m => `
    <tr onclick="navigate(event, '#feature/${m.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${m.title}</td>
      <td style="color:#fcb714; font-weight:bold;">${m.missingTests}</td>
      <td style="color:#ef4444; font-weight:bold;">${m.missingBugs}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No missing coverage</td></tr>';

  // Top Gaps
  featureGaps.sort((a, b) => b.count - a.count);
  topGapsTable.innerHTML = featureGaps.slice(0, 10).map(g => `
    <tr onclick="navigate(event, '#feature/${g.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${g.title}</td>
      <td style="font-weight:bold;">${g.count} artifacts</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No gaps declared</td></tr>';

  // Gaps Inventory
  const reasons = Object.entries(gapReasons).map(([reason, d]) => ({
    reason,
    count: d.count,
    features: Array.from(d.features).join(', ')
  })).sort((a, b) => b.count - a.count);

  gapsInventoryTable.innerHTML = reasons.map(r => `
    <tr>
      <td style="color:#fcb714; max-width:300px;">${r.reason}</td>
      <td style="font-weight:bold;">${r.count}</td>
      <td style="font-size:0.75rem; color:var(--text-muted);">${r.features}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No gap reasons found</td></tr>';

  if (typeof d3 !== 'undefined') {
    const wordWords = Object.entries(cloudWordCounts).map(([text, count]) => ({ text, count }));
    const sentenceWords = Object.entries(cloudSentenceCounts).map(([text, count]) => ({ text, count }));
    if (sentenceWords.length > 0) renderGapCloud('gapSentenceCloud', sentenceWords);
    if (wordWords.length > 0) renderGapCloud('gapWordCloud', wordWords);
  }
}

function renderGapCloud(containerId, words) {
  const container = document.getElementById(containerId);
  if (!container) return;
  container.innerHTML = '';

  const width = container.offsetWidth || 600;
  const height = 400;

  const data = words.map(w => ({ text: w.text, size: 12 + (Math.min(w.count, 50) * 4) }));

  const layout = d3.layout.cloud()
    .size([width, height])
    .words(data)
    .padding(5)
    .rotate(() => (~~(Math.random() * 2) * 90))
    .font("Impact")
    .fontSize(d => d.size)
    .on("end", (words) => {
      const svg = d3.select("#" + containerId).append("svg")
        .attr("width", layout.size()[0])
        .attr("height", layout.size()[1])
        .append("g")
        .attr("transform", "translate(" + layout.size()[0] / 2 + "," + layout.size()[1] / 2 + ")");

      const texts = svg.selectAll("text")
        .data(words)
        .enter().append("text")
        .style("font-size", "0px")
        .style("font-family", "Impact")
        .style("fill", () => d3.schemeTableau10[Math.floor(Math.random() * 10)])
        .attr("text-anchor", "middle")
        .attr("transform", d => "translate(" + [d.x, d.y] + ")rotate(" + d.rotate + ")")
        .text(d => d.text);

      texts.transition()
        .duration(1000)
        .style("font-size", d => d.size + "px");
    });

  layout.start();
}

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


function renderPlatformIcon(platform) {
  const p = platform.toLowerCase();
  if (p.includes('apple') || p.includes('ios') || p.includes('mac') || p.includes('iphone')) return '🍎';
  if (p.includes('android')) return '🤖';
  if (p.includes('windows')) return '🪟';
  if (p.includes('web') || p.includes('browser')) return '🌐';
  return '📱';
}
function renderPriorityIcon(priority) {
  const p = priority.toLowerCase();
  let color = '#8b9eb0';
  let svg = '';

  if (p === 'highest') {
    color = '#ff4d4d';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12M12 11L5 18M12 11L19 18" stroke-width="2.5" />';
  } else if (p === 'high') {
    color = '#ff8533';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12" stroke-width="2" />';
  } else if (p === 'medium') {
    color = '#fcb714';
    svg = '<path d="M5 12H19" stroke-width="3" />';
  } else if (p === 'low') {
    color = '#3b82f6';
    svg = '<path d="M12 5V19M12 19L5 12M12 19L19 12" stroke-width="2" />';
  } else {
    svg = '<circle cx="12" cy="12" r="4" fill="currentColor" />';
  }

  return `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="${color}" style="vertical-align: middle; margin-right: 4px;">${svg}</svg>`;
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
      (feature) => `<tr onclick="navigate(event, '#feature/${feature.id}')" style="cursor:pointer;">
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

function renderFeatureDetail(data, featureId) {
  const container = document.getElementById('featureDetailView');
  const feature = data.features.find(f => f.id === featureId);
  if (!feature) {
    container.innerHTML = '<h1>Feature not found</h1>';
    return;
  }

  let artifacts = [...feature.artifacts];

  // Search
  if (_detailSearchText) {
    const s = _detailSearchText.toLowerCase();
    artifacts = artifacts.filter(a => 
      a.title.toLowerCase().includes(s) || 
      (a.steps || []).some(step => step.toLowerCase().includes(s)) ||
      (a.expected || []).some(exp => exp.toLowerCase().includes(s)) ||
      (a.platforms || []).some(p => p.toLowerCase().includes(s))
    );
  }

  // Filtering
  if (_detailFilter === 'covered') artifacts = artifacts.filter(a => a.isCovered);
  if (_detailFilter === 'missing') artifacts = artifacts.filter(a => !a.isCovered);
  if (_detailFilter === 'critical') artifacts = artifacts.filter(a => !a.isCovered && (a.priority === 'High' || a.priority === 'Highest'));

  // Sorting
  artifacts.sort((a, b) => {
    let va, vb;
    switch (_detailSort) {
      case 'priority':
        va = PRIORITY_MAP[a.priority] || 0;
        vb = PRIORITY_MAP[b.priority] || 0;
        return vb - va; // Default High to Low
      case 'createdAt':
        va = a.createdAt || '';
        vb = b.createdAt || '';
        break;
      case 'updatedAt':
        va = a.updatedAt || a.createdAt || '';
        vb = b.updatedAt || b.createdAt || '';
        break;
      case 'status':
        va = a.isCovered ? 1 : 0;
        vb = b.isCovered ? 1 : 0;
        break;
      default: return 0;
    }
    if (va < vb) return 1;
    if (va > vb) return -1;
    return 0;
  });

  container.innerHTML = `
    <div class="detail-header">
      <button class="back-btn" onclick="navigate(event, '')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
        Back to Dashboard
      </button>
      <h1 class="detail-title">${feature.title}</h1>
      <div class="detail-meta">
        <span>ID: <strong>${feature.id}</strong></span>
        <span>Created: <strong>${feature.createdAt}</strong></span>
        ${feature.updatedAt ? `<span>Updated: <strong>${feature.updatedAt}</strong></span>` : ''}
      </div>
      <div class="detail-desc">${feature.description || 'No description provided.'}</div>
    </div>

    <section class="charts-row" style="margin-bottom: 2rem; grid-template-columns: 1fr 1fr;">
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Use Cases Coverage</h2>
             <span class="subtitle">${feature.useCasesCovered} / ${feature.useCases} (${feature.useCases > 0 ? ((feature.useCasesCovered/feature.useCases)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailUCChart"></canvas></div>
      </article>
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Bugs Coverage</h2>
             <span class="subtitle">${feature.bugsCovered} / ${feature.bugs} (${feature.bugs > 0 ? ((feature.bugsCovered/feature.bugs)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailBugChart"></canvas></div>
      </article>
    </section>

    <div class="detail-controls card" style="display:grid; grid-template-columns: 1fr auto; grid-template-rows: auto auto; padding:1.25rem; gap:1rem; margin-bottom: 2rem; border-radius:12px; background:var(--bg-card); border:1px solid var(--border);">
       <!-- Column 1: Search & Results -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.01); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.75rem;"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
         <input type="text" id="detailSearch" value="${_detailSearchText}" placeholder="Search artifacts by title, steps, or content..." 
                oninput="_detailSearchText=this.value; renderFeatureDetail(window._lastData, '${featureId}')"
                style="background:transparent; border:none; color:#fff; padding:1rem 0; width:100%; outline:none; font-size:0.9rem;">
       </div>
       <!-- TOP RIGHT: Results -->
       <div style="grid-column: 2; display:flex; align-items:center; padding:0 1rem; color:var(--text-muted); font-size:0.8rem; white-space:nowrap; border-bottom:1px solid transparent;">
         <strong>${artifacts.length}</strong> &nbsp;results
       </div>
       <!-- BOTTOM LEFT: Filter -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"></polygon></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Filter</span>
         <select id="filterSelect" onchange="_detailFilter=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="all" ${_detailFilter==='all'?'selected':''}>All</option>
           <option value="covered" ${_detailFilter==='covered'?'selected':''}>Covered</option>
           <option value="missing" ${_detailFilter==='missing'?'selected':''}>Missing</option>
           <option value="critical" ${_detailFilter==='critical'?'selected':''}>Critical</option>
         </select>
       </div>
       <!-- BOTTOM RIGHT: Sort -->
       <div style="grid-column: 2; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><path d="M3 12h18M3 6h18M3 18h18"></path></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Sort</span>
         <select id="sortSelect" onchange="_detailSort=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="priority" ${_detailSort==='priority'?'selected':''}>Priority</option>
           <option value="createdAt" ${_detailSort==='createdAt'?'selected':''}>Created</option>
           <option value="updatedAt" ${_detailSort==='updatedAt'?'selected':''}>Updated</option>
           <option value="status" ${_detailSort==='status'?'selected':''}>Coverage</option>
         </select>
       </div>
    </div>


    <div class="artifact-section">
      <div class="artifact-grid">
        ${artifacts.map(a => {
          const isHigh = a.priority === 'High' || a.priority === 'Highest';
          const isCritical = isHigh && !a.isCovered;
          return `
          <div class="artifact-card ${isCritical ? 'critical-missing' : ''}">
            <div class="artifact-head">
              <div style="display:flex; align-items:center; gap:0.75rem">
                <span class="badge ${a.type.toLowerCase().includes('bug') ? 'badge-bug' : 'badge-usecase'}">${a.type}</span>
                <span class="artifact-title">${a.title}</span>
              </div>
              <div style="display:flex; gap:0.5rem">
                ${isCritical ? '<span class="badge badge-critical">CRITICAL</span>' : ''}
                <span class="badge ${a.isCovered ? 'badge-covered' : 'badge-missing'}">${a.isCovered ? 'Covered' : 'Missing'}</span>
              </div>
            </div>
            <div class="artifact-body">
              <div style="display:flex; gap:1.5rem; margin-bottom: 0.5rem; font-size:0.8rem; color:var(--text-muted); align-items:center;">
                <div style="display:flex; align-items:center;">ID: <strong style="color:#fff; margin-left:0.4rem; margin-right:1rem;">${a.id}</strong></div>
                <div style="display:flex; align-items:center;">Priority: ${renderPriorityIcon(a.priority)} <strong style="color:#fff">${a.priority}</strong></div>
                ${a.platforms && a.platforms.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Platforms:</span>
                    ${a.platforms.map(p => `<div style="display:inline-flex; align-items:center; margin-right:0.6rem; font-size:0.7rem;">${renderPlatformIcon(p)} <span style="color:#fff; text-transform:uppercase; letter-spacing:0.02em;">${p}</span></div>`).join('')}
                  </div>
                ` : ''}
                ${a.tags && a.tags.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Tags:</span>
                    ${a.tags.map(t => `<span class="badge" style="background:rgba(252,183,20,0.1); color:#fcb714; border:1px solid rgba(252,183,20,0.2); font-size:0.65rem; text-transform:uppercase; padding: 2px 6px;">${t}</span>`).join('')}
                  </div>
                ` : ''}
                <div>Created: <strong>${a.createdAt}</strong></div>
                ${a.updatedAt ? `<div>Updated: <strong>${a.updatedAt}</strong></div>` : ''}
              </div>
              ${a.isCovered && a.coverageLocations && a.coverageLocations.length > 0 ? `
                <div style="position: absolute; bottom: 1.25rem; right: 1.25rem; display:flex; flex-wrap:wrap; gap:0.5rem; justify-content: flex-end;">
                  ${a.coverageLocations.map(loc => `
                    <a href="vscode://file/${loc.path}:${loc.line}" class="back-btn" style="margin:0; padding:4px 8px; font-size:0.75rem; text-decoration:none; display:inline-flex; align-items:center; gap:0.4rem; background:rgba(0,122,204,0.1); border-color:rgba(0,122,204,0.3); color:#4fc1ff;">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M23.15 2.58L19.8 1.45c-.32-.11-.66.1-.66.44v5.45c0 .12-.05.23-.14.31L13 13.7l-3.3-3.04c-.16-.14-.4-.14-.56 0L1 17.72c-.12.11-.12.3 0 .41l3.3 3.04c.16.14.4.14.56 0l1.24-1.14 7.22-6.66c.09-.08.14-.19.14-.31V6.26l6.68-6.16c.16-.14.4-.14.56 0l2.45 2.26c.12.11.12.3 0 .41L18.46 9c-.16.15-.16.4 0 .55l4.69 4.31c.12.11.12.3 0 .41l-2.45 2.26c-.16.14-.4.14-.56 0L13 12.23V17.74c0 .34.34.55.66.44l3.35-1.13c.12-.04.23-.04.35 0l2.45.83c.32.11.66-.1.66-.44V3.02c0-.12-.05-.23-.14-.31l-.18-.13z"/></svg>
                      ${loc.path.split('/').pop().split('\\').pop()}:L${loc.line}
                    </a>
                  `).join('')}
                </div>
              ` : ''}
              ${a.steps && a.steps.length ? `
                <div style="font-weight:600; color:var(--text-muted); font-size:0.8rem; margin-top:0.75rem; text-transform:uppercase;">Steps</div>
                <ol class="steps-list">
                  ${a.steps.map(s => `<li>${s}</li>`).join('')}
                </ol>
              ` : ''}
              ${a.expected && a.expected.length ? `
                <div class="expected-section">
                  <div class="expected-title">Expected Result</div>
                  <ul class="steps-list" style="list-style-type: disc">
                    ${a.expected.map(e => `<li>${e}</li>`).join('')}
                  </ul>
                </div>
              ` : ''}
              ${!a.isCovered && a.coverageGapReason ? `
                <div style="margin-top:1rem; padding:0.75rem; background:rgba(252,183,20,0.05); border-left:3px solid var(--accent); border-radius:4px;">
                  <div style="font-weight:600; color:var(--accent); font-size:0.75rem; text-transform:uppercase; margin-bottom:0.25rem;">Coverage Gap Reason</div>
                  <div style="font-size:0.85rem; color:var(--text-blue); font-style:italic;">"${a.coverageGapReason}"</div>
                </div>
              ` : ''}
            </div>
          </div>
        `;
      }).join('')}
        ${artifacts.length === 0 ? '<div class="card" style="padding:2rem; text-align:center; color:var(--text-muted)">No artifacts match the selected criteria.</div>' : ''}
      </div>
    </div>
  `;

  renderDetailCharts(feature);
  
  const searchInput = document.getElementById('detailSearch');
  if (searchInput) {
    searchInput.focus();
    searchInput.setSelectionRange(searchInput.value.length, searchInput.value.length);
  }
}

function renderDetailCharts(feature) {
  if (_detailUCChart) _detailUCChart.destroy();
  if (_detailBugChart) _detailBugChart.destroy();

  const animationConfig = {
    duration: 1200,
    easing: 'easeOutElastic',
    delay: (context) => context.dataIndex * 300
  };

  const ctxUC = document.getElementById('detailUCChart');
  if (ctxUC) {
    _detailUCChart = new Chart(ctxUC, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.useCasesCovered, feature.useCases - feature.useCasesCovered],
          backgroundColor: ['#10b981', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }

  const ctxBug = document.getElementById('detailBugChart');
  if (ctxBug) {
    _detailBugChart = new Chart(ctxBug, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.bugsCovered, feature.bugs - feature.bugsCovered],
          backgroundColor: ['#ef4444', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }
}




function handleRouting(data) {
  window.scrollTo(0, 0);
  const hash = window.location.hash || '#dashboard';
  const dashboard = document.getElementById('dashboardView');
  const detail = document.getElementById('featureDetailView');
  const inventory = document.getElementById('inventoryView');
  const tags = document.getElementById('tagsView');
  const gaps = document.getElementById('gapsView');
  
  [dashboard, detail, inventory, tags, gaps].forEach(v => { if(v) v.style.display = 'none'; });
  
  document.querySelectorAll('.nav-item').forEach(link => {
    const href = link.getAttribute('href');
    link.classList.toggle('active', href === hash || (hash === '#dashboard' && href === '#dashboard'));
  });

  if (hash.startsWith('#feature/')) {
    detail.style.display = 'block';
    renderFeatureDetail(data, hash.replace('#feature/', ''));
  } else if (hash === '#inventory') {
    inventory.style.display = 'block';
    renderInventory(data);
  } else if (hash === '#tags') {
    if (tags) tags.style.display = 'block';
    renderTagsView(data);
  } else if (hash === '#gaps') {
    if (gaps) gaps.style.display = 'block';
    renderGapsView(data);
  } else {
    dashboard.style.display = 'block';
    renderCharts(data);
  }
}

function renderTagsView(data) {
  const tagTable = document.getElementById('tagsTableBody');
  const platformTable = document.getElementById('platformsTableBody');
  if (!tagTable || !platformTable) return;

  const tagCounts = {};
  const platformCounts = {};

  data.features.forEach(f => {
    (f.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    (f.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
    (f.artifacts || []).forEach(a => {
        (a.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
        (a.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    });
  });

  const tags = Object.entries(tagCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);
  const platforms = Object.entries(platformCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);

  tagTable.innerHTML = tags.map(item => `
    <tr>
      <td style="color:#fcb714;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No tags found</td></tr>';

  platformTable.innerHTML = platforms.map(item => `
    <tr>
      <td style="color:#a5c8ff;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No platforms found</td></tr>';

  renderTagsViewCharts(tags, platforms);
}

function renderTagsViewCharts(tags, platforms) {
  _tagsViewCharts.forEach(c => c.destroy());
  _tagsViewCharts = [];

  const createChart = (id, items, color) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _tagsViewCharts.push(new Chart(ctx, {
      type: 'bar',
      data: {
        labels: items.map(i => i.name),
        datasets: [{
          data: items.map(i => i.count),
          backgroundColor: color,
          borderRadius: 4
        }]
      },
      options: {
        indexAxis: 'y',
        animation: { duration: 1000, easing: 'easeOutQuart' },
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
          y: { grid: { display: false }, ticks: { color: '#fff' } }
        },
        maintainAspectRatio: false
      }
    }));
  };

  createChart('platformsChart', platforms, '#a5c8ff');
  createChart('tagsChart', tags, '#fcb714');
}

function renderInventory(data) {
  if (!_searchIndex && typeof FlexSearch !== 'undefined') {
    _searchIndex = new FlexSearch.Document({
      document: {
        id: "id",
        index: ["title", "description", "content"],
        store: ["id"]
      },
      tokenize: "forward"
    });
    
    data.features.forEach(f => {
       const content = (f.artifacts || []).map(a => 
         `${a.title} ${(a.steps || []).join(' ')} ${(a.expected || []).join(' ')} ${(a.tags || []).join(' ')}`
       ).join(' ');
       const tagContent = (f.tags || []).join(' ');
       const platformContent = [...(f.platforms || []), ...(f.artifacts || []).flatMap(a => a.platforms || [])].join(' ');
       _searchIndex.add({
          id: f.id,
          title: f.title,
          description: f.description || "",
          content: `${content} ${tagContent} ${platformContent}`
       });
    });
  }

  renderInventoryCharts(data);
  renderInventoryTable(data);
  
  const search = document.getElementById('invSearch');
  if (search) {
    search.oninput = (e) => renderInventoryTable(data, e.target.value);
  }
}

function renderInventoryCharts(data) {
  _invCharts.forEach(c => c.destroy());
  _invCharts = [];
  
  const features = data.features;
  const labels = features.map(f => f.title);
  const colors = features.map((_, i) => `hsl(${(i * 360 / features.length) % 360}, 65%, 50%)`);

  const animationConfig = {
    duration: 200,
    easing: 'easeOutQuart',
    delay: (context) => context.dataIndex * 20
  };

  const createChart = (id, label, values, total) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _invCharts.push(new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: labels,
        datasets: [{ data: values, backgroundColor: colors, borderWidth: 0 }]
      },
      options: {
        animation: animationConfig,
        plugins: { 
          legend: { 
            display: true, 
            position: 'right', 
            labels: { color: '#8b9eb0', boxWidth: 12, padding: 10, font: { size: 10 } } 
          },
          tooltip: { callbacks: { label: (ctx) => `${ctx.label}: ${ctx.raw} ${label}` } }
        },
        maintainAspectRatio: false,
        cutout: '65%'
      },
      plugins: [{
        id: 'centerText',
        beforeDraw: (chart) => {
          const { ctx, width, height } = chart;
          ctx.save();
          ctx.font = 'bold 1.2rem Roboto';
          ctx.fillStyle = '#fff';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const centerLeft = chart.chartArea.left + (chart.chartArea.right - chart.chartArea.left) / 2;
          ctx.fillText(total, centerLeft, height / 2 - 10);
          ctx.font = '0.7rem Roboto';
          ctx.fillStyle = '#8b9eb0';
          ctx.fillText('TOTAL', centerLeft, height / 2 + 15);
          ctx.restore();
        }
      }]
    }));
  };

  const totalUC = features.reduce((sum, f) => sum + f.useCases, 0);
  const totalBugs = features.reduce((sum, f) => sum + f.bugs, 0);
  const totalCovUC = features.reduce((sum, f) => sum + f.useCasesCovered, 0);
  const totalCovBugs = features.reduce((sum, f) => sum + f.bugsCovered, 0);

  createChart('invUCChart', 'Use Cases', features.map(f => f.useCases), totalUC);
  createChart('invBugChart', 'Bugs', features.map(f => f.bugs), totalBugs);
  createChart('invCoveredUCChart', 'Covered UC', features.map(f => f.useCasesCovered), totalCovUC);
  createChart('invCoveredBugChart', 'Covered Bugs', features.map(f => f.bugsCovered), totalCovBugs);
}

function renderInventoryTable(data, filter = '') {
  const tbody = document.getElementById('invRows');
  if (!tbody) return;

  let matched = data.features;
  if (filter && _searchIndex) {
    const results = _searchIndex.search(filter);
    const ids = new Set();
    results.forEach(r => r.result.forEach(id => ids.add(id)));
    matched = data.features.filter(feat => ids.has(feat.id));
  }

  const countEl = document.getElementById('featCount');
  if (countEl) countEl.textContent = `(${matched.length} features)`;

  tbody.innerHTML = matched.map(feat => `
    <tr onclick="navigate(event, '#feature/${feat.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${feat.title}</td>
      <td style="color:var(--text-muted); font-size:0.85rem;">${feat.description || '-'}</td>
      <td style="color:var(--text-muted); font-size:0.8rem;">${feat.updatedAt || feat.createdAt}</td>
      <td>
        <div style="font-size:0.8rem;">UC: <strong>${feat.useCasesCovered}/${feat.useCases}</strong></div>
        <div style="font-size:0.8rem;">Bugs: <strong>${feat.bugsCovered}/${feat.bugs}</strong></div>
      </td>
    </tr>
  `).join('');
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
  _dashboardCharts.forEach(c => c.destroy());
  _dashboardCharts = [];

  let delayed = false;
  const animationConfig = {
    onComplete: () => { delayed = true; },
    delay: (context) => {
      let delay = 0;
      if (context.type === 'data' && context.mode === 'default' && !delayed) {
        delay = context.dataIndex * 300 + context.datasetIndex * 100;
      }
      return delay;
    }
  };

  const chartConfig = {
    animation: animationConfig,
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
    _dashboardCharts.push(new Chart(useCaseCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const featureCanvas = document.getElementById('featureGrowthChart');
  if (featureCanvas) {
    _dashboardCharts.push(new Chart(featureCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Features', data: data.growth.features, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 }
        ]
      },
      options: chartConfig
    }));
  }

  const bugCanvas = document.getElementById('bugGrowthChart');
  if (bugCanvas) {
    _dashboardCharts.push(new Chart(bugCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Bugs', data: data.growth.bugs, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const progressCanvas = document.getElementById('featureCoverageChart');
  if (progressCanvas) {
    _dashboardCharts.push(new Chart(progressCanvas, {
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
        animation: animationConfig,
        plugins: { 
          legend: { display: true, position: 'top', align: 'end', labels: { boxWidth: 12, color: '#8b9eb0' } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
          y: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
        },
        maintainAspectRatio: false
      }
    }));
  }
}

function bootstrap() {
  const data = loadData();
  window._lastData = data;
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

  window.addEventListener('hashchange', () => handleRouting(data));
  handleRouting(data);
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
                last_modified_at: None,
                description: "desc".to_string(),
            },
            tags: vec![],
            platforms: vec![],
            related_features: vec![],
            artifacts: vec![Artifact {
                id: "ucc-feat-1".to_string(),
                artifact_type: None,
                created_at: "2026-05-10".to_string(),
                updated_at: None,
                last_modified_at: None,
                title: "Use case".to_string(),
                priority: Priority::High,
                related: vec![],
                platforms: vec![],
                steps: vec![],
                expected: vec![],
                tags: vec![],
                coverage_gap_reason: None,
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
        assert!(output_dir.join("vendor/d3.v7.min.js").exists());
        assert!(output_dir.join("vendor/d3.layout.cloud.js").exists());
        assert!(output_dir.join("vendor/chart.js").exists());
        assert!(output_dir.join("vendor/flexsearch.bundle.js").exists());

        let html =
            fs::read_to_string(output_dir.join("index.html")).expect("html should be readable");
        assert!(html.contains("UCC Report"));
        assert!(html.contains("./vendor/d3.v7.min.js"));
        assert!(html.contains("./vendor/chart.js"));

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

    #[test]
    fn build_report_returns_formatted_string() {
        use super::build_report;
        let report = build_report(5, 10);
        assert_eq!(report, "Use case coverage: 5/10 (50.00%)");
    }

    #[test]
    fn is_bug_identifies_bugs_and_regressions() {
        use super::is_bug;
        assert!(is_bug(Some("bug")));
        assert!(is_bug(Some("BUG")));
        assert!(is_bug(Some("regression")));
        assert!(is_bug(Some("Regression")));
        assert!(!is_bug(Some("usecase")));
        assert!(!is_bug(None));
    }

    #[test]
    fn build_growth_data_handles_multiple_months() {
        use super::{build_growth_data, current_year_month};
        use std::path::PathBuf;
        use use_case_coverage_core::domain::{
            Artifact, ArtifactCoverageIndex, FeatureDocument, FeatureMetadata, Priority,
        };

        let (now_y, now_m) = current_year_month();
        let date_str = format!("{now_y}-{now_m:02}-10");

        let features = vec![FeatureDocument {
            source_path: PathBuf::from("feat1.ucc"),
            schema_version: "1.0".to_string(),
            feature: FeatureMetadata {
                id: "feat-1".to_string(),
                title: "Feat 1".to_string(),
                created_at: date_str.clone(),
                updated_at: None,
                last_modified_at: None,
                description: "desc".to_string(),
            },
            tags: vec![],
            platforms: vec![],
            related_features: vec![],
            artifacts: vec![
                Artifact {
                    id: "ucc-1".to_string(),
                    artifact_type: None,
                    created_at: date_str.clone(),
                    updated_at: None,
                    last_modified_at: None,
                    title: "UC 1".to_string(),
                    priority: Priority::High,
                    related: vec![],
                    platforms: vec![],
                    steps: vec![],
                    expected: vec![],
                    tags: vec![],
                    coverage_gap_reason: None,
                },
                Artifact {
                    id: "bug-1".to_string(),
                    artifact_type: Some("bug".to_string()),
                    created_at: date_str,
                    updated_at: None,
                    last_modified_at: None,
                    title: "Bug 1".to_string(),
                    priority: Priority::High,
                    related: vec![],
                    platforms: vec![],
                    steps: vec![],
                    expected: vec![],
                    tags: vec![],
                    coverage_gap_reason: None,
                },
            ],
        }];

        let mut index = ArtifactCoverageIndex::default();
        index.by_artifact_id.insert("ucc-1".to_string(), vec![]);
        index.by_artifact_id.insert("bug-1".to_string(), vec![]);

        let growth = build_growth_data(&features, &index);

        assert_eq!(growth["months"].as_array().unwrap().len(), 12);
        assert_eq!(growth["useCases"].as_array().unwrap()[11].as_u64().unwrap(), 1);
        assert_eq!(growth["bugs"].as_array().unwrap()[11].as_u64().unwrap(), 1);
        assert_eq!(growth["coveredUseCases"].as_array().unwrap()[11].as_u64().unwrap(), 1);
        assert_eq!(growth["coveredBugs"].as_array().unwrap()[11].as_u64().unwrap(), 1);
    }

    #[test]
    fn parse_year_month_handles_valid_and_invalid_dates() {
        use super::parse_year_month;
        assert_eq!(parse_year_month("2026-05-10"), Some((2026, 5)));
        assert_eq!(parse_year_month("2026-13-10"), None);
        assert_eq!(parse_year_month("invalid"), None);
        assert_eq!(parse_year_month("2026-05"), Some((2026, 5)));
    }

    #[test]
    fn month_short_name_returns_correct_names() {
        use super::month_short_name;
        assert_eq!(month_short_name(1), "Jan");
        assert_eq!(month_short_name(12), "Dec");
        assert_eq!(month_short_name(13), "??");
    }
}
