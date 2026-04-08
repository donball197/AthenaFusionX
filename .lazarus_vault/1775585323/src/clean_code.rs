/// Returns a vector containing only the even integers from the input vector.
/// Refactored from a manual loop to an idiomatic iterator pattern following clippy suggestions.
pub fn get_even_numbers(numbers: Vec<i32>) -> Vec<i32> {
    numbers
        .into_iter()
        .filter(|n| n % 2 == 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_even_numbers() {
        assert_eq!(get_even_numbers(vec![1, 2, 3, 4, 5, 6]), vec![2, 4, 6]);
    }

    #[test]
    fn test_all_odd() {
        assert_eq!(get_even_numbers(vec![1, 3, 5]), vec![]);
    }

    #[test]
    fn test_empty() {
        let empty: Vec<i32> = vec![];
        assert_eq!(get_even_numbers(empty), vec![]);
    }
}