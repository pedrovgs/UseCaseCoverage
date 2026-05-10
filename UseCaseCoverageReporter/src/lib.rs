#![forbid(unsafe_code)]

use use_case_coverage_core::coverage_percentage;

/// Builds a tiny human-readable coverage report.
#[must_use]
pub fn build_report(covered: u32, total: u32) -> String {
    let percentage = coverage_percentage(covered, total);
    format!("Use case coverage: {covered}/{total} ({percentage:.2}%)")
}

#[cfg(test)]
mod tests {
    use super::build_report;

    #[test]
    fn simple_report_test() {
        let report = build_report(3, 4);
        assert!(report.contains("75.00%"));
    }
}
