#[cfg(test)]
mod tests {
    use crate::hiram_core;

    #[test]
    fn test_integrity_logic() {
        // This ensures the AI doesn't remove the core purpose of your kernel
        let initial_state = 100;
        let result = hiram_core::integrity_check(initial_state);
        assert!(result >= initial_state, "Evolution reduced system integrity!");
    }

    #[test]
    fn test_sync_constant() {
        // Ensures the AI doesn't mess with global constants needed for the Duet
        assert_eq!(hiram_core::SYNC_VERSION, 1.0);
    }
}
