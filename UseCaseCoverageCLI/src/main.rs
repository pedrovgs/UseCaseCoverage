#![forbid(unsafe_code)]

use std::process::ExitCode;

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
            "By Pedro Gomez - https://github.com/sponsors/pedrovgs\n\n",
            "\x1b[1;38;5;208mUsage:\x1b[0m\n",
            "  \x1b[38;5;159mucc [OPTIONS] [COMMAND]\x1b[0m\n\n",
            "\x1b[1;38;5;208mOptions:\x1b[0m\n",
            "  \x1b[38;5;120m-h, --help\x1b[0m    Show this help message\n\n",
            "\x1b[1;38;5;208mCommands:\x1b[0m\n",
            "  \x1b[38;5;117mreport\x1b[0m        Generate an HTML report with features, use cases, and bugs analysis\n",
            "  \x1b[38;5;117mlint\x1b[0m          Explore and validate .ucc files, ensuring the format is correct and nothing is missing\n"
        ),
        version = env!("CARGO_PKG_VERSION")
    )
}

fn run(args: &[String]) -> Result<String, String> {
    if args.len() == 1 {
        return Ok(help_message());
    }

    match args[1].as_str() {
        "-h" | "--help" => Ok(help_message()),
        "lint" => Err("The 'lint' command is not implemented yet.".to_string()),
        "report" => Err("The 'report' command is not implemented yet.".to_string()),
        unknown => Err(format!("Unknown command or option: {unknown}\n\n{}", help_message())),
    }
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
    use super::{help_message, run};

    #[test]
    fn simple_cli_test() {
        let args = vec!["ucc".to_string()];
        let output = run(&args).expect("CLI should return help");
        assert!(output.contains("Commands:"));
    }

    #[test]
    fn short_help_option_prints_help() {
        let args = vec!["ucc".to_string(), "-h".to_string()];
        let output = run(&args).expect("CLI should return help");
        assert_eq!(output, help_message());
    }

    #[test]
    fn long_help_option_prints_help() {
        let args = vec!["ucc".to_string(), "--help".to_string()];
        let output = run(&args).expect("CLI should return help");
        assert_eq!(output, help_message());
    }
}
