
pub fn fibonacci(n:u32)->u32 {

    match n {

        0 => 0,

        1 => 1,

        n =>
            fibonacci(n-1)
            +
            fibonacci(n-2)

    }

}


#[cfg(test)]
mod tests {

use super::*;


#[test]
fn fibonacci_validation(){

    assert_eq!(fibonacci(0),0);
    assert_eq!(fibonacci(1),1);
    assert_eq!(fibonacci(10),55);

}

}

