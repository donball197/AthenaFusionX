use nexus_edge_minion::rope::{apply_rope_scalar, AttentionMode, RoPEConfig};

#[test]
fn test_rope_position_zero_invariant() {
    let config = RoPEConfig::new(AttentionMode::Global);
    let mut vec = vec![1.0f32; 256];
    let original = vec.clone();

    apply_rope_scalar(&mut vec, 0, &config);

    // At position 0, angle is 0 -> cos(0)=1, sin(0)=0 -> vector must remain unchanged
    for i in 0..256 {
        assert!(
            (vec[i] - original[i]).abs() < 1e-6,
            "Position 0 failed at index {}: got {}, expected {}",
            i, vec[i], original[i]
        );
    }
}

#[test]
fn test_rope_global_partial_rotary_bounds() {
    let config = RoPEConfig::new(AttentionMode::Global); // theta = 1e6, rotary_dim = 64
    config.validate();

    let mut vec: Vec<f32> = (0..256).map(|i| i as f32 + 1.0).collect();
    let original = vec.clone();

    apply_rope_scalar(&mut vec, 5, &config);

    // First 64 dimensions must be modified (rotated)
    let mut modified_count = 0;
    for i in 0..64 {
        if (vec[i] - original[i]).abs() > 1e-5 {
            modified_count += 1;
        }
    }
    assert!(
        modified_count > 0,
        "Global attention failed: expected rotation in the first 64 dimensions."
    );

    // Dimensions 64..256 must remain strictly invariant (identity rotation)
    for i in 64..256 {
        assert!(
            (vec[i] - original[i]).abs() < 1e-6,
            "Global attention invariant failure at index {}: got {}, expected {}",
            i, vec[i], original[i]
        );
    }
}

#[test]
fn test_rope_sliding_full_rotary() {
    let config = RoPEConfig::new(AttentionMode::Sliding); // theta = 1e4, rotary_dim = 256
    config.validate();

    let mut vec: Vec<f32> = (0..256).map(|i| i as f32 + 1.0).collect();
    let original = vec.clone();

    apply_rope_scalar(&mut vec, 10, &config);

    // All dimensions across 0..256 should be modified under full rotary
    let mut modified_count = 0;
    for i in 0..256 {
        if (vec[i] - original[i]).abs() > 1e-5 {
            modified_count += 1;
        }
    }
    assert!(
        modified_count > 200,
        "Sliding attention failed: expected full-dimensional rotation across head_dim."
    );
}

#[test]
#[should_panic(expected = "FATAL: Head dimension must be exactly 256")]
fn test_rope_invalid_config_panic() {
    let bad_config = RoPEConfig {
        head_dim: 128,
        rotary_dim: 64,
        theta: 1_000_000.0,
    };
    let mut vec = vec![1.0f32; 128];
    apply_rope_scalar(&mut vec, 1, &bad_config);
}
