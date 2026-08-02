
pub fn calculate_score(value: i32) -> i32 {

    // Intentional defect:
    // should add 10 but subtracts 10

    value - 10
}


#[cfg(test)]
mod tests {

    use super::*;


    #[test]
    fn test_score() {

        assert_eq!(calculate_score(10),20);

    }
}
