use runtime_core::math_utils::fibonacci;

#[test]
fn test_fibonacci_base_cases() {
    assert_eq!(fibonacci(0), Some(0));
    assert_eq!(fibonacci(1), Some(1));
}

#[test]
fn test_fibonacci_known_values() {
    assert_eq!(fibonacci(2), Some(1));
    assert_eq!(fibonacci(3), Some(2));
    assert_eq!(fibonacci(4), Some(3));
    assert_eq!(fibonacci(5), Some(5));
    assert_eq!(fibonacci(10), Some(55));
    assert_eq!(fibonacci(20), Some(6765));
}

#[test]
fn test_fibonacci_overflow() {
    // The 93rd Fibonacci number is the largest that fits in a u64
    // Fib(93) = 12,200,160,415,121,876,738
    assert!(fibonacci(93).is_some());
    // Fib(94) should overflow u64
    assert_eq!(fibonacci(94), None);
}