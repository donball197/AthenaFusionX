#[cfg(test)]
mod tests {
    use crate::kernel::safe_vmo::SafeVmo;

    #[test]
    fn test_vmo_boundary_conditions() {
        // Test 1: Zero-sized allocation should fail gracefully
        let zero_vmo = SafeVmo::new(0);
        assert!(zero_vmo.is_err(), "Zero-sized VMO should not be allowed");

        // Test 2: Normal VMO OOB Writes
        let vmo = SafeVmo::new(1024).unwrap();
        let data = [1u8; 100];

        // Write starting at 1000 with 100 bytes (extends to 1100, past 1024 limit)
        assert!(vmo.write(1000, &data).is_err(), "OOB write should return error");

        // Test 3: Integer Overflow Attack
        // usize::MAX will wrap around if added to data.len(), previously bypassing the check
        assert!(
            vmo.write(usize::MAX, &data).is_err(),
            "Integer overflow OOB write should return error"
        );

        // Test 4: Normal VMO OOB Reads
        let mut read_buf = [0u8; 100];
        assert!(vmo.read_into(1000, &mut read_buf).is_err(), "OOB read should return error");
        assert!(
            vmo.read_into(usize::MAX, &mut read_buf).is_err(),
            "Integer overflow OOB read should return error"
        );
    }
}
