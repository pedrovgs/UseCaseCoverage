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
            "Generate use case coverage analysis reports and validate .ucc specifications\n\n",
            "\x1b[1;38;5;208mUsage:\x1b[0m\n",
            "  \x1b[38;5;159mucc [OPTIONS] [COMMAND]\x1b[0m\n\n",
            "\x1b[1;38;5;208mOptions:\x1b[0m\n",
            "  \x1b[38;5;120m-h, --help\x1b[0m           Show this help message\n",
            "  \x1b[38;5;120m-i, --input <path>\x1b[0m    Root directory to scan for .ucc files (default: current directory)\n",
            "  \x1b[38;5;120m-o, --output <path>\x1b[0m   Output file (lint) or directory (report) (default: stdout for lint, <current-dir>/.ucc/ for report)\n",
            "  \x1b[38;5;120m--as, --additional-sources <path>\x1b[0m  Additional directories to scan for .ucc and test files (repeatable)\n",
            "  \x1b[38;5;120m-s, --single\x1b[0m          Disable recursive .ucc file discovery (only scan top-level directory)\n\n",
            "\x1b[1;38;5;208mCommands:\x1b[0m\n",
            "  \x1b[38;5;117mreport\x1b[0m        Generate an HTML report with features, use cases, and bugs analysis\n",
            "  \x1b[38;5;117mlint\x1b[0m          Explore and validate .ucc files, ensuring the format is correct and nothing is missing\n\n",
            "By Pedro Gomez - https://github.com/sponsors/pedrovgs\n"
        ),
        version = env!("CARGO_PKG_VERSION")
    )
}

fn run_lint(
    roots: &[std::path::PathBuf],
    output: Option<&Path>,
    recursive: bool,
) -> Result<String, String> {
    let start = std::time::Instant::now();
    println!("🔍 Scanning and linting .ucc files...");
    let lint_results = lint_ucc_formats(roots, recursive).map_err(|error| error.to_string())?;
    let duration = start.elapsed();
    let result = format_lint_results(lint_results);

    let is_error = result.is_err();
    let result_text = match result {
        Ok(t) => format!("{t}\n✨ Done in {duration:.2?}"),
        Err(e) => format!("{e}\n❌ Failed in {duration:.2?}"),
    };

    if let Some(output_path) = output {
        std::fs::write(output_path, &result_text).map_err(|e| e.to_string())?;
        Ok(format!("Lint results written to {}", output_path.display()))
    } else if is_error {
        Err(result_text)
    } else {
        Ok(result_text)
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

fn run_report(
    roots: &[std::path::PathBuf],
    output: Option<&Path>,
    recursive: bool,
) -> Result<String, String> {
    let start = std::time::Instant::now();
    println!("🔍 Linting .ucc files...");
    let lint_results = lint_ucc_formats(roots, recursive).map_err(|error| error.to_string())?;

    let invalid_count = lint_results.iter().filter(|result| !result.is_valid).count();
    if invalid_count > 0 {
        let lint_output = format_lint_results(lint_results).unwrap_or_else(|error| error);
        return Err(format!("Cannot generate report because {invalid_count} .ucc file(s) are invalid.\n\n{lint_output}"));
    }

    println!("📊 Collecting features and artifacts...");
    let features = collect_features_from(roots, recursive).map_err(|error| error.to_string())?;
    println!("🧪 Finding artifact coverage in codebase...");
    let coverage_index =
        find_artifact_coverage(roots, &features).map_err(|error| error.to_string())?;

    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let repo_name = cwd.file_name().and_then(|name| name.to_str()).unwrap_or("Unknown");
    let date = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let default_output = cwd.join(".ucc").join("reports").join(date);
    let output_dir = output.unwrap_or(&default_output);
    println!("🔨 Generating HTML report in {}...", output_dir.display());
    generate_html_report(output_dir, repo_name, &features, &lint_results, &coverage_index)
        .map_err(|error| error.to_string())?;

    let duration = start.elapsed();
    let report_path = output_dir.join("index.html");
    Ok(format!(
        "Report generated successfully at:\n{}\n✨ Done in {duration:.2?}",
        report_path.display()
    ))
}

fn parse_args(
    args: &[String],
) -> (Option<String>, Option<String>, Option<String>, Vec<String>, bool) {
    let mut input = None;
    let mut output = None;
    let mut command = None;
    let mut additional_sources = Vec::new();
    let mut recursive = true;
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
            "-h" | "--help" if command.is_none() => {
                command = Some(args[i].clone());
            }
            s if !s.starts_with('-') && command.is_none() => {
                command = Some(s.to_string());
            }
            "--as" | "--additional-sources" => {
                i += 1;
                if let Some(path) = args.get(i) {
                    additional_sources.push(path.clone());
                }
            }
            "-s" | "--single" => {
                recursive = false;
            }
            _ => {}
        }
        i += 1;
    }

    (input, output, command, additional_sources, recursive)
}

fn dispatch(
    roots: &[std::path::PathBuf],
    output: Option<&Path>,
    command: Option<&str>,
    recursive: bool,
) -> Result<String, String> {
    match command {
        None | Some("-h" | "--help") => Ok(help_message()),
        Some("lint") => run_lint(roots, output, recursive),
        Some("report") => run_report(roots, output, recursive),
        Some(unknown) => Err(format!("Unknown command or option: {unknown}\n\n{}", help_message())),
    }
}

#[allow(dead_code)]
fn run_with_root(args: &[String], root: &Path) -> Result<String, String> {
    let (_, output, command, additional_sources, recursive) = parse_args(args);
    let mut roots = vec![root.to_path_buf()];
    for source in additional_sources {
        let source_path = root.join(source);
        if !source_path.exists() {
            eprintln!(
                "Warning: additional source path '{}' does not exist. Skipping.",
                source_path.display()
            );
            continue;
        }
        roots.push(source_path);
    }
    dispatch(&roots, output.as_deref().map(Path::new), command.as_deref(), recursive)
}

fn run(args: &[String]) -> Result<String, String> {
    let (input, output, command, additional_sources, recursive) = parse_args(args);
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut roots = Vec::new();

    let root = input.map_or_else(|| cwd.clone(), PathBuf::from);
    let root = if root.is_absolute() { root } else { cwd.join(root) };
    roots.push(root);

    for source in additional_sources {
        let source_path = PathBuf::from(source);
        let source_path =
            if source_path.is_absolute() { source_path } else { cwd.join(source_path) };
        if !source_path.exists() {
            eprintln!(
                "Warning: additional source path '{}' does not exist. Skipping.",
                source_path.display()
            );
            continue;
        }
        roots.push(source_path);
    }

    dispatch(&roots, output.as_deref().map(Path::new), command.as_deref(), recursive)
}

fn author_message() -> String {
    format!(
        "\n\x1b[1;38;5;208m💌 All the feedback is welcome!\x1b[0m\n\
        I want to maintain the tool free from any type of tracking so it's completely anonymous.\n\
        If you are using the app or if you have any feedback for me, please let me know at \x1b[1;38;5;159mpedrovicente.gomez@gmail.com\x1b[0m. Thanks! ✨\n"
    )
}

fn print_result(result: Result<String, String>) -> ExitCode {
    match result {
        Ok(output) => {
            println!("{output}");
            println!("{}", author_message());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            println!("{}", author_message());
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

    #[test]
    fn report_command_generates_report_with_timestamp_folder_by_default() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        fs::write(root.join("feature.ucc"), sample_ucc()).expect("valid ucc should be written");

        let args = vec!["ucc".to_string(), "report".to_string()];
        let output = run_with_root(&args, root).expect("report should succeed");

        // The output should contain the default path which includes a timestamp:
        // .ucc/reports/YYYY-MM-DD_HH-MM-SS/index.html
        let re =
            regex::Regex::new(r"\.ucc/reports/\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}/index\.html")
                .unwrap();
        assert!(re.is_match(&output), "Output '{output}' did not match expected pattern");
    }

    #[test]
    fn additional_sources_combines_files_from_multiple_dirs() {
        let temp = tempdir().expect("tempdir should be created");
        let root_a = temp.path().join("a");
        let root_b = temp.path().join("b");
        fs::create_dir_all(&root_a).unwrap();
        fs::create_dir_all(&root_b).unwrap();

        fs::write(root_a.join("a.ucc"), sample_ucc_with_id("feat-a", "ucc-a")).unwrap();
        fs::write(root_b.join("b.ucc"), sample_ucc_with_id("feat-b", "ucc-b")).unwrap();

        let args = vec![
            "ucc".to_string(),
            "lint".to_string(),
            "--as".to_string(),
            root_b.to_string_lossy().to_string(),
        ];
        let output = run_with_root(&args, &root_a).expect("lint should succeed");
        assert!(output.contains("Linted 2 .ucc file(s)"));
    }

    #[test]
    fn single_flag_excludes_nested_ucc_files() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        fs::write(root.join("root.ucc"), sample_ucc_with_id("feat-root", "ucc-root"))
            .expect("root ucc should be written");
        fs::create_dir(root.join("nested")).expect("nested dir should be created");
        fs::write(root.join("nested/nested.ucc"), sample_ucc_with_id("feat-nested", "ucc-nested"))
            .expect("nested ucc should be written");

        // Without -s, both are found
        let args_recursive = vec!["ucc".to_string(), "lint".to_string()];
        let output_recursive = run_with_root(&args_recursive, root).expect("lint should succeed");
        assert!(output_recursive.contains("Linted 2 .ucc file(s)"));

        // With -s, only one is found
        let args_single = vec!["ucc".to_string(), "lint".to_string(), "-s".to_string()];
        let output_single = run_with_root(&args_single, root).expect("lint should succeed");
        assert!(output_single.contains("Linted 1 .ucc file(s)"));
    }

    #[test]
    fn lint_command_writes_to_output_file() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let output_file = root.join("lint_results.txt");

        fs::write(root.join("valid.ucc"), sample_ucc()).expect("valid ucc should be written");

        let args = vec![
            "ucc".to_string(),
            "lint".to_string(),
            "-o".to_string(),
            output_file.to_string_lossy().to_string(),
        ];
        let output = run_with_root(&args, root).expect("lint should succeed");

        assert!(output.contains("written to"));
        assert!(output_file.exists());
        let content = fs::read_to_string(output_file).expect("output file should be readable");
        assert!(content.contains("Linted 1 .ucc file(s)"));
    }

    #[test]
    fn format_lint_results_handles_empty_list() {
        use super::format_lint_results;
        let result = format_lint_results(vec![]);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("No .ucc files were found"));
    }

    #[test]
    fn run_with_non_existing_additional_source_skips_it() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        let args = vec![
            "ucc".to_string(),
            "lint".to_string(),
            "--as".to_string(),
            "non_existing_dir".to_string(),
        ];
        let output = run_with_root(&args, root).expect("lint should succeed");
        assert!(output.contains("No .ucc files were found"));
    }

    #[test]
    fn dispatch_handles_unknown_command() {
        use super::dispatch;
        let result = dispatch(&[], None, Some("unknown"), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown command or option"));
    }

    #[test]
    fn run_returns_error_for_unknown_command() {
        use super::run;
        let result = run(&["ucc".to_string(), "unknown".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn print_result_handles_ok_and_err() {
        use super::print_result;
        // This will print to stdout/stderr, which is fine in tests
        print_result(Ok("ok".to_string()));
        print_result(Err("err".to_string()));
    }

    #[test]
    fn format_lint_results_handles_unknown_location() {
        use super::format_lint_results;
        use use_case_coverage_core::domain::{UccLintIssue, UccLintResult};
        let results = vec![UccLintResult {
            file_path: std::path::PathBuf::from("feat.ucc"),
            is_valid: false,
            issue: Some(UccLintIssue {
                message: "error".to_string(),
                line: None,
                column: None,
                suggestion: None,
            }),
        }];
        let result = format_lint_results(results);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown location"));
    }

    fn sample_ucc_with_id(feat_id: &str, artifact_id: &str) -> String {
        format!(
            r#"schema_version: "1.0"

feature:
  id: {feat_id}
  title: Feature {feat_id}
  created_at: "2026-05-10"
  description: >
    Sample feature.

artifacts:
  - id: {artifact_id}
    created_at: "2026-05-10"
    title: A use case
    priority: high
    steps:
      - Step one
    expected:
      - Expected one
"#
        )
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
