pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative_and_positive() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    fn test_add_zeros() {
        assert_eq!(add(0, 0), 0);
    }

    #[test]
    fn test_multiply_positive() {
        assert_eq!(multiply(3, 4), 12);
    }

    #[test]
    fn test_multiply_negative_and_positive() {
        assert_eq!(multiply(-2, 3), -6);
    }

    #[test]
    fn test_multiply_by_zero() {
        assert_eq!(multiply(0, 5), 0);
    }
}