#![forbid(unsafe_code)]

use std::process::ExitCode;

use use_case_coverage_core::coverage_percentage;
use use_case_coverage_reporter::build_report;

fn parse_arg(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("Invalid {label}: '{value}'. Expected an unsigned integer."))
}

fn run(args: &[String]) -> Result<String, String> {
    if args.len() != 3 {
        return Err("Usage: UseCaseCoverageCLI <covered> <total>".to_string());
    }

    let covered = parse_arg(&args[1], "covered")?;
    let total = parse_arg(&args[2], "total")?;

    let _ = coverage_percentage(covered, total);

    Ok(build_report(covered, total))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    match run(&args) {
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

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn simple_cli_test() {
        let args = vec!["UseCaseCoverageCLI".to_string(), "1".to_string(), "2".to_string()];
        let output = run(&args).expect("CLI should produce a report");
        assert!(output.contains("50.00%"));
    }
}
