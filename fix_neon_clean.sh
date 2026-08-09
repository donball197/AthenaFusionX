#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Rewriting neon_math.rs with strict attribute formatting..."

cat << 'INNER' > crates/nexus_edge_minion/src/neon_math.rs
//! High-performance ARM64 NEON matvec for BF16 → F32

use half::bf16;
use std::arch::aarch64::*;

#[inline(always)]
pub fn matvec_bf16_to_f32(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    assert_eq!(w.len(), m * n);
    assert_eq!(x.len(), n);
    assert_eq!(y.len(), m);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        if is_bf16_supported() {
            matvec_bf16_hw(w, x, y, m, n);
        } else {
            matvec_bf16_neon_v2(w, x, y, m, n);
        }
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

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn is_bf16_supported() -> bool {
    std::arch::is_aarch64_feature_detected!("bf16")
}

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
        let mut acc0 = vdupq_n_f32(0.0);
        let mut acc1 = vdupq_n_f32(0.0);
        let mut acc2 = vdupq_n_f32(0.0);
        let mut acc3 = vdupq_n_f32(0.0);

        let row_ptr = w.as_ptr().add(i * n);
        let x_ptr = x.as_ptr();
        let mut j = 0usize;

        while j + 16 <= n {
            core::arch::aarch64::_prefetch(
                row_ptr.add(j + 64) as *const i8,
                core::arch::aarch64::_PREFETCH_READ,
                core::arch::aarch64::_PREFETCH_LOCALITY3,
            );

            let vw0 = vld1q_u16(row_ptr.add(j) as *const u16);
            let vx0 = vld1q_u16(x_ptr.add(j) as *const u16);

            let w0l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vw0), 16));
            let w0h = vreinterpretq_f32_u32(vshll_high_n_u16(vw0, 16));
            let x0l = vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(vx0), 16));
            let x0h = vreinterpretq_f32_u32(vshll_high_n_u16(vx0, 16));

            acc0 = vfmaq_f32(acc0, w0l, x0l);
            acc1 = vfmaq_f32(acc1, w0h, x0h);

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

        let sum01 = vaddq_f32(acc0, acc1);
        let sum23 = vaddq_f32(acc2, acc3);
        let sum = vaddq_f32(sum01, sum23);
        let mut total = vaddvq_f32(sum);

        while j < n {
            total += w[i * n + j].to_f32() * x[j].to_f32();
            j += 1;
        }

        y[i] = total;
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon,bf16")]
unsafe fn matvec_bf16_hw(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    for i in 0..m {
        let mut acc = vdupq_n_f32(0.0);
        let row_ptr = w.as_ptr().add(i * n) as *const bfloat16_t;
        let x_ptr = x.as_ptr() as *const bfloat16_t;
        let mut j = 0usize;

        while j + 8 <= n {
            core::arch::aarch64::_prefetch(
                (row_ptr as *const u8).add(j * 2 + 128) as *const i8,
                core::arch::aarch64::_PREFETCH_READ,
                core::arch::aarch64::_PREFETCH_LOCALITY3,
            );

            let vw = vld1q_bf16(row_ptr.add(j));
            let vx = vld1q_bf16(x_ptr.add(j));

            acc = vbfdotq_f32(acc, vw, vx);

            j += 8;
        }

        let mut total = vaddvq_f32(acc);

        while j < n {
            total += w[i * n + j].to_f32() * x[j].to_f32();
            j += 1;
        }

        y[i] = total;
    }
}

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
INNER

echo ">> [NEXUS] Re-running Clippy..."
cargo clippy --workspace --all-targets -j 3 -- -D warnings
