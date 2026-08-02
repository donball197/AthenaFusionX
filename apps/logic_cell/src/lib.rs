#[no_mangle]
pub extern "C" fn evaluate_signal(signal: i32) -> i32 {
    if signal > 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_signal() {
        // Positive signals should return 1
        assert_eq!(evaluate_signal(1), 1);
        assert_eq!(evaluate_signal(42), 1);
        assert_eq!(evaluate_signal(i32::MAX), 1);

        // Zero and negative signals should return 0
        assert_eq!(evaluate_signal(0), 0);
        assert_eq!(evaluate_signal(-1), 0);
        assert_eq!(evaluate_signal(-42), 0);
        assert_eq!(evaluate_signal(i32::MIN), 0);
    }
}
