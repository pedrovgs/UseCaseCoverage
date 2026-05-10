#![forbid(unsafe_code)]

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use use_case_coverage_core::domain::UccLintResult;
use use_case_coverage_core::{collect_features_from, find_artifact_coverage, lint_ucc_formats};
use use_case_coverage_reporter::generate_html_report;

#[must_use]
fn help_message() -> String {
    format!(
        concat!(
            "\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;220m✓\x1b[0m\x1b[38;5;25m]\x1b[0m\n",
            "\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;220m✓\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\n",
            "\x1b[38;5;25m[\x1b[0m\x1b[38;5;220m✓\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\x1b[38;5;25m[\x1b[0m\x1b[38;5;25m■\x1b[0m\x1b[38;5;25m]\x1b[0m\n\n",
            "\x1b[1;38;5;226mucc v{version}\x1b[0m\n\n",
            "\x1b[1mUse Case Coverage CLI\x1b[0m\n",
            "Generate use case coverage analysis reports and vlaidate .ucc specifications\n\n",
            "\x1b[1;38;5;208mUsage:\x1b[0m\n",
            "  \x1b[38;5;159mucc [OPTIONS] [COMMAND]\x1b[0m\n\n",
            "\x1b[1;38;5;208mOptions:\x1b[0m\n",
            "  \x1b[38;5;120m-h, --help\x1b[0m           Show this help message\n",
            "  \x1b[38;5;120m-i, --input <path>\x1b[0m    Root directory to scan for .ucc files (default: current directory)\n",
            "  \x1b[38;5;120m-o, --output <path>\x1b[0m   Output file (lint) or directory (report) (default: stdout for lint, <current-dir>/.ucc/ for report)\n\n",
            "\x1b[1;38;5;208mCommands:\x1b[0m\n",
            "  \x1b[38;5;117mreport\x1b[0m        Generate an HTML report with features, use cases, and bugs analysis\n",
            "  \x1b[38;5;117mlint\x1b[0m          Explore and validate .ucc files, ensuring the format is correct and nothing is missing\n\n",
            "By Pedro Gomez - https://github.com/sponsors/pedrovgs\n"
        ),
        version = env!("CARGO_PKG_VERSION")
    )
}

fn run_lint(root: &Path, output: Option<&Path>) -> Result<String, String> {
    let lint_results = lint_ucc_formats(root).map_err(|error| error.to_string())?;
    let result = format_lint_results(lint_results);

    if let Some(output_path) = output {
        let text = match &result {
            Ok(t) | Err(t) => t,
        };
        std::fs::write(output_path, text).map_err(|e| e.to_string())?;
        result.map(|_| format!("Lint results written to {}", output_path.display()))
    } else {
        result
    }
}

fn format_lint_results(lint_results: Vec<UccLintResult>) -> Result<String, String> {
    if lint_results.is_empty() {
        return Ok("No .ucc files were found in the current directory tree.".to_string());
    }

    let valid = lint_results.iter().filter(|result| result.is_valid).count();
    let invalid = lint_results.len().saturating_sub(valid);

    let mut output = String::new();
    let _ = writeln!(
        output,
        "Linted {} .ucc file(s): {} valid, {} invalid",
        lint_results.len(),
        valid,
        invalid
    );

    for result in lint_results {
        if result.is_valid {
            let _ = writeln!(output, "[OK] {}", result.file_path.display());
        } else if let Some(issue) = result.issue {
            let mut location = String::new();
            if let Some(line) = issue.line {
                let _ = write!(location, "line {line}");
            }
            if let Some(column) = issue.column {
                if !location.is_empty() {
                    location.push_str(", ");
                }
                let _ = write!(location, "column {column}");
            }
            if location.is_empty() {
                location.push_str("unknown location");
            }

            let _ = writeln!(
                output,
                "[ERROR] {} ({location})\n  {}",
                result.file_path.display(),
                issue.message
            );

            if let Some(suggestion) = issue.suggestion {
                let _ = writeln!(output, "  Suggestion: {suggestion}");
            }
        }
    }

    if invalid > 0 {
        return Err(output);
    }

    Ok(output)
}

fn run_report(root: &Path, output: Option<&Path>) -> Result<String, String> {
    let lint_results = lint_ucc_formats(root).map_err(|error| error.to_string())?;

    let invalid_count = lint_results.iter().filter(|result| !result.is_valid).count();
    if invalid_count > 0 {
        let lint_output = format_lint_results(lint_results).unwrap_or_else(|error| error);
        return Err(format!("Cannot generate report because {invalid_count} .ucc file(s) are invalid.\n\n{lint_output}"));
    }

    let features = collect_features_from(root).map_err(|error| error.to_string())?;
    let coverage_index =
        find_artifact_coverage(root, &features).map_err(|error| error.to_string())?;

    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let repo_name = cwd.file_name().and_then(|name| name.to_str()).unwrap_or("Unknown");
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let default_output = cwd.join(".ucc").join(timestamp);
    let output_dir = output.unwrap_or(&default_output);
    generate_html_report(output_dir, repo_name, &features, &lint_results, &coverage_index)
        .map_err(|error| error.to_string())?;

    let report_path = output_dir.join("index.html");
    Ok(format!("Report generated successfully at:\n{}", report_path.display()))
}

fn parse_args(args: &[String]) -> (Option<String>, Option<String>, Option<String>) {
    let mut input = None;
    let mut output = None;
    let mut command = None;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input" => {
                i += 1;
                input = args.get(i).cloned();
            }
            "-o" | "--output" => {
                i += 1;
                output = args.get(i).cloned();
            }
            "-h" | "--help" => {
                if command.is_none() {
                    command = Some(args[i].clone());
                }
            }
            s if !s.starts_with('-') && command.is_none() => {
                command = Some(s.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    (input, output, command)
}

fn dispatch(root: &Path, output: Option<&Path>, command: Option<&str>) -> Result<String, String> {
    match command {
        None | Some("-h") | Some("--help") => Ok(help_message()),
        Some("lint") => run_lint(root, output),
        Some("report") => run_report(root, output),
        Some(unknown) => Err(format!("Unknown command or option: {unknown}\n\n{}", help_message())),
    }
}

#[allow(dead_code)]
fn run_with_root(args: &[String], root: &Path) -> Result<String, String> {
    let (_, output, command) = parse_args(args);
    dispatch(root, output.as_deref().map(Path::new), command.as_deref())
}

fn run(args: &[String]) -> Result<String, String> {
    let (input, output, command) = parse_args(args);
    let root = match input {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    dispatch(&root, output.as_deref().map(Path::new), command.as_deref())
}

fn print_result(result: Result<String, String>) -> ExitCode {
    match result {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    print_result(run(&args))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{help_message, run_with_root};

    #[test]
    fn simple_cli_test() {
        let args = vec!["ucc".to_string()];
        let output =
            run_with_root(&args, std::path::Path::new(".")).expect("CLI should return help");
        assert!(output.contains("Commands:"));
    }

    #[test]
    fn short_help_option_prints_help() {
        let args = vec!["ucc".to_string(), "-h".to_string()];
        let output =
            run_with_root(&args, std::path::Path::new(".")).expect("CLI should return help");
        assert_eq!(output, help_message());
    }

    #[test]
    fn long_help_option_prints_help() {
        let args = vec!["ucc".to_string(), "--help".to_string()];
        let output =
            run_with_root(&args, std::path::Path::new(".")).expect("CLI should return help");
        assert_eq!(output, help_message());
    }

    #[test]
    fn lint_command_reports_invalid_files() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        fs::write(root.join("valid.ucc"), sample_ucc()).expect("valid ucc should be written");
        fs::write(root.join("broken.ucc"), "schema_version: [")
            .expect("broken ucc should be written");

        let args = vec!["ucc".to_string(), "lint".to_string()];
        let error = run_with_root(&args, root).expect_err("lint should fail");

        assert!(error.contains("invalid"));
        assert!(error.contains("broken.ucc"));
    }

    #[test]
    fn report_command_generates_report_when_lint_passes() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        fs::write(root.join("feature.ucc"), sample_ucc()).expect("valid ucc should be written");
        fs::write(root.join("feature.spec.ts"), "test('covers ucc-001', () => {});\n")
            .expect("test file should be written");

        let output_dir = root.join(".ucc");
        let args = vec![
            "ucc".to_string(),
            "report".to_string(),
            "-o".to_string(),
            output_dir.to_string_lossy().to_string(),
        ];
        let output = run_with_root(&args, root).expect("report should succeed");

        assert!(output.contains("index.html"));
        assert!(output_dir.join("index.html").exists());
    }

    #[test]
    fn report_command_fails_if_lint_fails() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        fs::write(root.join("broken.ucc"), "schema_version: [")
            .expect("broken ucc should be written");

        let args = vec!["ucc".to_string(), "report".to_string()];
        let error = run_with_root(&args, root).expect_err("report should fail");

        assert!(error.contains("Cannot generate report"));
        assert!(error.contains("broken.ucc"));
    }

    fn sample_ucc() -> &'static str {
        r#"schema_version: "1.0"

feature:
  id: feat-1
  title: Feature One
  created_at: "2026-05-10"
  description: >
    Sample feature.

artifacts:
  - id: ucc-001
    created_at: "2026-05-10"
    title: A use case
    priority: high
    steps:
      - Step one
    expected:
      - Expected one
"#
    }
}
