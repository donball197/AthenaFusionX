/// Calculates the nth Fibonacci number using checked arithmetic to prevent overflow.
/// Returns None if the result exceeds the capacity of u128.
pub fn fibonacci(n: u32) -> Option<u128> {
    if n == 0 {
        return Some(0);
    }
    if n == 1 {
        return Some(1);
    }

    let mut a: u128 = 0;
    let mut b: u128 = 1;

    for _ in 2..=n {
        let next = a.checked_add(b)?;
        a = b;
        b = next;
    }

    Some(b)
}