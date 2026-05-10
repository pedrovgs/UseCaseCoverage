#![forbid(unsafe_code)]

/// Calculates simple use case coverage as a percentage in the `0.0..=100.0` range.
#[must_use]
pub fn coverage_percentage(covered: u32, total: u32) -> f64 {
    if total == 0 {
        return 0.0;
    }

    (f64::from(covered.min(total)) / f64::from(total)) * 100.0
}

#[cfg(test)]
mod tests {
    use super::coverage_percentage;
    use proptest::prelude::*;

    #[test]
    fn simple_coverage_test() {
        assert!((coverage_percentage(5, 10) - 50.0).abs() < f64::EPSILON);
    }

    proptest! {
        #[test]
        fn coverage_is_bounded(covered in 0_u32..10_000, total in 0_u32..10_000) {
            let value = coverage_percentage(covered, total);
            prop_assert!((0.0..=100.0).contains(&value));
        }
    }
}
