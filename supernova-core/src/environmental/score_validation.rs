//! Environmental Score Validation Module
//!
//! This module provides validation and outlier detection for environmental scores
//! to prevent manipulation attacks on the green mining incentive system.

use std::collections::VecDeque;

/// Environmental score validation configuration
#[derive(Debug, Clone)]
pub struct EnvironmentalScoreValidator {
    /// Minimum valid score (default: 0)
    min_score: f64,
    /// Maximum valid score (default: 100)
    max_score: f64,
    /// History of recent scores for outlier detection
    score_history: VecDeque<f64>,
    /// Maximum history size for outlier detection
    max_history_size: usize,
    /// Z-score threshold for outlier detection (default: 3.0)
    outlier_threshold: f64,
}

impl Default for EnvironmentalScoreValidator {
    fn default() -> Self {
        Self {
            min_score: 0.0,
            max_score: 100.0,
            score_history: VecDeque::with_capacity(1000),
            max_history_size: 1000,
            outlier_threshold: 3.0,
        }
    }
}

impl EnvironmentalScoreValidator {
    /// Create a new environmental score validator
    pub fn new(min_score: f64, max_score: f64, outlier_threshold: f64) -> Self {
        Self {
            min_score,
            max_score,
            score_history: VecDeque::with_capacity(1000),
            max_history_size: 1000,
            outlier_threshold,
        }
    }

    /// Validate an environmental score
    ///
    /// # Arguments
    /// * `score` - Environmental score to validate (should be 0-100)
    ///
    /// # Returns
    /// * `Ok(())` - Score is valid
    /// * `Err(String)` - Score is invalid with reason
    pub fn validate_score(&self, score: f64) -> Result<(), String> {
        // Check range
        if score < self.min_score {
            return Err(format!(
                "Environmental score {} below minimum {}",
                score, self.min_score
            ));
        }

        if score > self.max_score {
            return Err(format!(
                "Environmental score {} above maximum {}",
                score, self.max_score
            ));
        }

        // Check for NaN or infinity
        if !score.is_finite() {
            return Err(format!("Environmental score is not finite: {}", score));
        }

        Ok(())
    }

    /// Validate score and check for outliers
    ///
    /// # Arguments
    /// * `score` - Environmental score to validate
    ///
    /// # Returns
    /// * `Ok(())` - Score is valid and not an outlier
    /// * `Err(String)` - Score is invalid or outlier
    pub fn validate_with_outlier_detection(&mut self, score: f64) -> Result<(), String> {
        // First validate range
        self.validate_score(score)?;

        // Check for outliers if we have enough history
        if self.score_history.len() >= 10 {
            if self.is_outlier(score) {
                return Err(format!(
                    "Environmental score {} is an outlier (z-score > {})",
                    score, self.outlier_threshold
                ));
            }
        }

        // Add to history
        self.add_to_history(score);

        Ok(())
    }

    /// Check if a score is an outlier using z-score
    fn is_outlier(&self, score: f64) -> bool {
        if self.score_history.is_empty() {
            return false;
        }

        // Calculate mean
        let mean: f64 = self.score_history.iter().sum::<f64>() / self.score_history.len() as f64;

        // Calculate standard deviation
        let variance: f64 = self.score_history
            .iter()
            .map(|s| (s - mean).powi(2))
            .sum::<f64>()
            / self.score_history.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return false; // All scores are the same
        }

        // Calculate z-score
        let z_score = (score - mean).abs() / std_dev;

        z_score > self.outlier_threshold
    }

    /// Add score to history
    fn add_to_history(&mut self, score: f64) {
        self.score_history.push_back(score);
        if self.score_history.len() > self.max_history_size {
            self.score_history.pop_front();
        }
    }

    /// Get statistics about score history
    pub fn get_statistics(&self) -> ScoreStatistics {
        if self.score_history.is_empty() {
            return ScoreStatistics {
                count: 0,
                mean: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
            };
        }

        let count = self.score_history.len();
        let mean: f64 = self.score_history.iter().sum::<f64>() / count as f64;
        let variance: f64 = self.score_history
            .iter()
            .map(|s| (s - mean).powi(2))
            .sum::<f64>()
            / count as f64;
        let std_dev = variance.sqrt();
        let min = self.score_history.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = self.score_history.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        ScoreStatistics {
            count,
            mean,
            std_dev,
            min,
            max,
        }
    }

    /// Clear score history
    pub fn clear_history(&mut self) {
        self.score_history.clear();
    }
}

/// Statistics about environmental scores
#[derive(Debug, Clone)]
pub struct ScoreStatistics {
    pub count: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_range_validation() {
        let validator = EnvironmentalScoreValidator::default();

        // Valid scores
        assert!(validator.validate_score(0.0).is_ok());
        assert!(validator.validate_score(50.0).is_ok());
        assert!(validator.validate_score(100.0).is_ok());

        // Invalid scores
        assert!(validator.validate_score(-1.0).is_err());
        assert!(validator.validate_score(101.0).is_err());
        assert!(validator.validate_score(f64::NAN).is_err());
        assert!(validator.validate_score(f64::INFINITY).is_err());
    }

    #[test]
    fn test_outlier_detection() {
        let mut validator = EnvironmentalScoreValidator::default();

        // Add normal scores (around 50)
        for _ in 0..20 {
            validator.add_to_history(50.0);
        }

        // Normal score should pass
        assert!(validator.validate_with_outlier_detection(52.0).is_ok());

        // Extreme outlier should fail
        assert!(validator.validate_with_outlier_detection(200.0).is_err());
    }

    #[test]
    fn test_statistics() {
        let mut validator = EnvironmentalScoreValidator::default();

        validator.add_to_history(50.0);
        validator.add_to_history(60.0);
        validator.add_to_history(70.0);

        let stats = validator.get_statistics();
        assert_eq!(stats.count, 3);
        assert!((stats.mean - 60.0).abs() < 0.1);
        assert_eq!(stats.min, 50.0);
        assert_eq!(stats.max, 70.0);
    }
}

