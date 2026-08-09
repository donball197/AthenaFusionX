#!/bin/bash
# forge_neon_stable.sh - Project Nexus Singularity v0.2.7
# Stable, high-performance NEON matvec (no unstable features)

set -euo pipefail

echo ">> [NEXUS] Forging stable optimized NEON substrate..."

mkdir -p crates/nexus_edge_minion/src

cat << 'EOF' > crates/nexus_edge_minion/src/neon_math.rs
//! Stable high-performance ARM64 NEON matvec (BF16 → F32)
//! - 4 accumulators
//! - 16-wide inner loop
//! - Clean vshll conversion
//! - Zero extra heap allocations
//! - Only stable intrinsics

use half::bf16;
use std::arch::aarch64::*;

/// Public entry point – safe wrapper around the unsafe kernel
#[inline(always)]
pub fn matvec_bf16_to_f32(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    assert_eq!(w.len(), m * n, "weight size mismatch");
    assert_eq!(x.len(), n, "input size mismatch");
    assert_eq!(y.len(), m, "output size mismatch");

    #[cfg(target_arch = "aarch64")]
    unsafe {
        matvec_bf16_neon_v2(w, x, y, m, n);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        for i in 0..m {
            let mut sum = 0.0f32;
            let row = i * n;
            for j in 0..n {
                sum += w[row + j].to_f32() * x[j].to_f32();
            }
            y[i] = sum;
        }
    }
}

/// Keep the old name working for existing call sites
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn matvec_bf16_to_f32_neon(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    matvec_bf16_to_f32(w, x, y, m, n);
}

// ----------------------------------------------------------------------
// Core kernel – stable intrinsics only
// ----------------------------------------------------------------------
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn matvec_bf16_neon_v2(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    for i in 0..m {
        // Four independent accumulators → better ILP
        let mut acc0 = vdupq_n_f32(0.0);
        let mut acc1 = vdupq_n_f32(0.0);
        let mut acc2 = vdupq_n_f32(0.0);
        let mut acc3 = vdupq_n_f32(0.0);

        let row_ptr = w.as_ptr().add(i * n);
        let x_ptr   = x.as_ptr();
        let mut j   = 0usize;

        // Process 16 BF16 elements per iteration
        while j + 16 <= n {
            // ---- first group of 8 ----
            let vw0 = vld1q_u16(row_ptr.add(j) as *const u16);
            let vx0 = vld1q_u16(x_ptr.add(j) as *const u16);

            // BF16 → F32 via shift-left-16 (one instruction each)
            let w0l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vw0), 16));
            let w0h = vreinterpretq_f32_u32(vshll_high_n_u16(vw0, 16));
            let x0l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vx0), 16));
            let x0h = vreinterpretq_f32_u32(vshll_high_n_u16(vx0, 16));

            acc0 = vfmaq_f32(acc0, w0l, x0l);
            acc1 = vfmaq_f32(acc1, w0h, x0h);

            // ---- second group of 8 ----
            let vw1 = vld1q_u16(row_ptr.add(j + 8) as *const u16);
            let vx1 = vld1q_u16(x_ptr.add(j + 8) as *const u16);

            let w1l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vw1), 16));
            let w1h = vreinterpretq_f32_u32(vshll_high_n_u16(vw1, 16));
            let x1l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vx1), 16));
            let x1h = vreinterpretq_f32_u32(vshll_high_n_u16(vx1, 16));

            acc2 = vfmaq_f32(acc2, w1l, x1l);
            acc3 = vfmaq_f32(acc3, w1h, x1h);

            j += 16;
        }

        // Horizontal reduction
        let sum01 = vaddq_f32(acc0, acc1);
        let sum23 = vaddq_f32(acc2, acc3);
        let sum   = vaddq_f32(sum01, sum23);
        let mut total = vaddvq_f32(sum);

        // Tiny scalar remainder
        while j < n {
            total += w[i * n + j].to_f32() * x[j].to_f32();
            j += 1;
        }

        y[i] = total;
    }
}
EOF

# Keep the rest of the files (kv_cache + main) from the previous good state
# (they do not need changes)

echo ">> [NEXUS] Stable optimized neon_math.rs written."
echo ""
echo "Now rebuild:"
echo "  cargo build -p nexus_edge_minion --release"
echo "  cargo run  -p nexus_edge_minion --release"
