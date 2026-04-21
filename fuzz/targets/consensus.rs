//! Fuzz harness: consensus difficulty adjustment.
//!
//! The adjustment function takes `current_target`, a slice of block
//! timestamps, and a slice of block heights. All three are adversarial.
//! The invariant under test is: `calculate_next_target` returns a `Result`
//! and never panics — regardless of out-of-order timestamps, zero-length
//! slices, or pathological values.

use afl::fuzz;
use supernova_core::consensus::difficulty::DifficultyAdjustment;

fn main() {
    fuzz!(|data: &[u8]| {
        if data.len() < 5 {
            return;
        }

        // First 4 bytes = current_target; 5th byte = split ratio for
        // timestamps vs heights.
        let current_target = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let split = usize::from(data[4]);
        let body = &data[5..];

        // Every 8 bytes → one u64. Partition into timestamps and heights.
        let pairs = body.len() / 8;
        if pairs == 0 {
            return;
        }
        let timestamps_len = (split.saturating_mul(pairs) / 256).min(pairs);
        let heights_len = pairs.saturating_sub(timestamps_len);

        let mut timestamps = Vec::with_capacity(timestamps_len);
        let mut heights = Vec::with_capacity(heights_len);

        for i in 0..timestamps_len {
            let start = i * 8;
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&body[start..start + 8]);
            timestamps.push(u64::from_le_bytes(buf));
        }
        for i in 0..heights_len {
            let start = (timestamps_len + i) * 8;
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&body[start..start + 8]);
            heights.push(u64::from_le_bytes(buf));
        }

        // Target invariant: no panic under any input.
        let da = DifficultyAdjustment::new();
        let _ = da.calculate_next_target(current_target, &timestamps, &heights);
    });
}
