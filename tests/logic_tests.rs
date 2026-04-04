#[cfg(test)]
mod tests {
    use nexus_singularity::hiram_core::Hiram;

    #[test]
    fn test_hiram_initialization() {
        let hiram = Hiram::new(3600);

        // It should start at 100% integrity
        assert_eq!(hiram.integrity, 100);
        assert!(hiram.last_verified.is_none());
        assert_eq!(hiram.verification_interval, 3600);
    }

    #[test]
    fn test_hiram_verify_execution() {
        let mut hiram = Hiram::new(3600);

        // Run the verify function. It shouldn't panic, and it should stamp the time.
        hiram.verify();
        assert!(hiram.last_verified.is_some());
    }
}
