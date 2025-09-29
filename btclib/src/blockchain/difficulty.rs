
/// Calculate difficulty from compact bits representation
pub fn calculate_difficulty_from_bits(bits: u32) -> f64 {
    let max_target = 0x1d00ffff_u32;
    let current_target = bits;
    
    // Convert to actual target values
    let max_target_value = compact_to_target(max_target);
    let current_target_value = compact_to_target(current_target);
    
    // Difficulty = max_target / current_target
    if current_target_value > 0.0 {
        max_target_value / current_target_value
    } else {
        1.0
    }
}

/// Convert compact bits to target value
fn compact_to_target(bits: u32) -> f64 {
    let exponent = (bits >> 24) & 0xff;
    let mantissa = bits & 0xffffff;
    
    if exponent <= 3 {
        (mantissa >> (8 * (3 - exponent))) as f64
    } else {
        (mantissa as f64) * 256_f64.powf((exponent - 3) as f64)
    }
}

/// Calculate network hashrate from difficulty and block time
pub fn calculate_hashrate(difficulty: f64, block_time_seconds: u64) -> u64 {
    // Hashrate = difficulty * 2^32 / block_time
    let hashrate = (difficulty * 4_294_967_296.0) / block_time_seconds as f64;
    hashrate as u64
}

/// Get difficulty adjustment ratio
pub fn get_difficulty_adjustment_ratio(actual_time: u64, target_time: u64) -> f64 {
    // Limit adjustment to 4x in either direction
    let ratio = actual_time as f64 / target_time as f64;
    ratio.max(0.25).min(4.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_difficulty_calculation() {
        // Genesis block difficulty
        let bits = 0x1d00ffff;
        let difficulty = calculate_difficulty_from_bits(bits);
        assert!((difficulty - 1.0).abs() < 0.0001);
        
        // Higher difficulty
        let bits = 0x1b0404cb;
        let difficulty = calculate_difficulty_from_bits(bits);
        assert!(difficulty > 1.0);
    }
    
    #[test]
    fn test_hashrate_calculation() {
        let difficulty = 1000.0;
        let block_time = 600; // 10 minutes
        let hashrate = calculate_hashrate(difficulty, block_time);
        assert!(hashrate > 0);
    }
} 