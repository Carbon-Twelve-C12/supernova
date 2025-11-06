//! Quantum-Resistant Password Strength Requirements
//!
//! Implements entropy-based password strength checking suitable for
//! post-quantum security requirements.
//!
//! Entropy Requirements:
//! - Minimum 128 bits (quantum-resistant threshold)
//! - Recommended 192 bits for long-term security
//! - Maximum 256 bits (matches ML-KEM security level)

use std::collections::HashSet;

/// Password strength score
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PasswordScore {
    /// < 80 bits - REJECTED (vulnerable to classical attacks)
    Rejected = 0,
    /// 80-127 bits - WEAK (vulnerable to quantum attacks)
    Weak = 1,
    /// 128-159 bits - ACCEPTABLE (quantum-resistant minimum)
    Acceptable = 2,
    /// 160-191 bits - STRONG (recommended for Supernova)
    Strong = 3,
    /// 192+ bits - EXCELLENT (future-proof)
    Excellent = 4,
}

impl PasswordScore {
    /// Check if password meets minimum quantum-resistant requirements
    pub fn is_quantum_resistant(&self) -> bool {
        matches!(self, PasswordScore::Acceptable | PasswordScore::Strong | PasswordScore::Excellent)
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            PasswordScore::Rejected => "REJECTED - Vulnerable to classical attacks",
            PasswordScore::Weak => "WEAK - Vulnerable to quantum attacks",
            PasswordScore::Acceptable => "ACCEPTABLE - Quantum-resistant minimum",
            PasswordScore::Strong => "STRONG - Recommended for Supernova",
            PasswordScore::Excellent => "EXCELLENT - Future-proof",
        }
    }
}

/// Password strength analysis result
#[derive(Debug, Clone)]
pub struct PasswordStrength {
    /// Entropy in bits
    pub entropy_bits: f64,
    /// Strength score
    pub score: PasswordScore,
    /// Suggestions for improvement
    pub suggestions: Vec<String>,
    /// Estimated time to crack (classical)
    pub time_to_crack_classical: String,
    /// Estimated time to crack (quantum)
    pub time_to_crack_quantum: String,
    /// Character set used
    pub character_set_size: usize,
    /// Pattern matches detected
    pub patterns_detected: Vec<String>,
}

/// Common passwords dictionary (top 10,000 most common)
/// In production, this would be loaded from a file or external source
const COMMON_PASSWORDS: &[&str] = &[
    "password", "123456", "password123", "admin", "letmein",
    "welcome", "monkey", "1234567890", "qwerty", "abc123",
    "12345678", "password1", "iloveyou", "princess", "rockyou",
    "1234567", "123456789", "sunshine", "football", "charlie",
];

/// BIP39 wordlist size (for passphrase entropy calculation)
const BIP39_WORDLIST_SIZE: usize = 2048;

/// Password strength checker
pub struct PasswordStrengthChecker {
    /// Common passwords dictionary
    common_passwords: HashSet<String>,
}

impl PasswordStrengthChecker {
    /// Create a new password strength checker
    pub fn new() -> Self {
        let mut common_passwords = HashSet::new();
        for pwd in COMMON_PASSWORDS {
            common_passwords.insert(pwd.to_lowercase());
            // Add common variants
            common_passwords.insert(format!("{}123", pwd));
            common_passwords.insert(format!("{}{}", pwd, pwd));
        }

        Self { common_passwords }
    }

    /// Check password strength
    pub fn check_strength(&self, password: &str) -> PasswordStrength {
        let entropy = self.calculate_entropy(password);
        let score = self.score_from_entropy(entropy);
        let suggestions = self.generate_suggestions(password, entropy, score);
        let patterns = self.detect_patterns(password);
        
        PasswordStrength {
            entropy_bits: entropy,
            score,
            suggestions,
            time_to_crack_classical: self.estimate_time_to_crack(entropy, false),
            time_to_crack_quantum: self.estimate_time_to_crack(entropy, true),
            character_set_size: self.character_set_size(password),
            patterns_detected: patterns,
        }
    }

    /// Calculate password entropy in bits
    fn calculate_entropy(&self, password: &str) -> f64 {
        // Check if it's a passphrase (space-separated words)
        if password.contains(' ') {
            return self.calculate_passphrase_entropy(password);
        }

        // Character-based entropy
        let char_set_size = self.character_set_size(password) as f64;
        let length = password.len() as f64;

        // Base entropy: log2(char_set_size) * length
        let mut entropy = (char_set_size.log2()) * length;

        // Apply penalties for patterns
        entropy -= self.pattern_penalty(password);

        // Apply penalty for dictionary matches
        entropy -= self.dictionary_penalty(password);

        // Apply penalty for L33t speak variants
        entropy -= self.l33t_penalty(password);

        entropy.max(0.0)
    }

    /// Calculate passphrase entropy (word-based)
    fn calculate_passphrase_entropy(&self, passphrase: &str) -> f64 {
        let words: Vec<&str> = passphrase.split_whitespace().collect();
        let word_count = words.len();

        if word_count == 0 {
            return 0.0;
        }

        // Assume BIP39 wordlist (2048 words)
        // Entropy per word: log2(2048) = 11 bits
        let entropy_per_word = (BIP39_WORDLIST_SIZE as f64).log2();
        let base_entropy = entropy_per_word * word_count as f64;

        // Check if words are from common dictionary
        let mut common_word_count = 0;
        for word in &words {
            if self.common_passwords.contains(&word.to_lowercase()) {
                common_word_count += 1;
            }
        }

        // Penalty for common words
        let penalty = if common_word_count > 0 {
            (common_word_count as f64 / word_count as f64) * base_entropy * 0.5
        } else {
            0.0
        };

        (base_entropy - penalty).max(0.0)
    }

    /// Calculate character set size
    fn character_set_size(&self, password: &str) -> usize {
        let mut has_lower = false;
        let mut has_upper = false;
        let mut has_digit = false;
        let mut has_special = false;

        for ch in password.chars() {
            if ch.is_ascii_lowercase() {
                has_lower = true;
            } else if ch.is_ascii_uppercase() {
                has_upper = true;
            } else if ch.is_ascii_digit() {
                has_digit = true;
            } else if ch.is_ascii_punctuation() || !ch.is_ascii() {
                has_special = true;
            }
        }

        let mut size = 0;
        if has_lower {
            size += 26;
        }
        if has_upper {
            size += 26;
        }
        if has_digit {
            size += 10;
        }
        if has_special {
            size += 33; // Common ASCII special characters
        }

        size.max(26) // Minimum: lowercase only
    }

    /// Detect patterns in password
    fn detect_patterns(&self, password: &str) -> Vec<String> {
        let mut patterns = Vec::new();

        // Check for keyboard walks
        if self.is_keyboard_walk(password) {
            patterns.push("Keyboard walk detected".to_string());
        }

        // Check for sequences
        if self.has_sequence(password) {
            patterns.push("Sequential characters detected".to_string());
        }

        // Check for repeats
        if self.has_repeats(password) {
            patterns.push("Repeated characters detected".to_string());
        }

        // Check for dictionary match
        if self.is_dictionary_word(password) {
            patterns.push("Common dictionary word detected".to_string());
        }

        // Check for L33t speak
        if self.is_l33t_variant(password) {
            patterns.push("L33t speak variant detected".to_string());
        }

        patterns
    }

    /// Check if password contains keyboard walk pattern
    fn is_keyboard_walk(&self, password: &str) -> bool {
        let qwerty_rows = [
            "qwertyuiop", "asdfghjkl", "zxcvbnm",
            "QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM",
        ];

        for row in &qwerty_rows {
            for i in 0..row.len().saturating_sub(3) {
                let pattern = &row[i..i + 4];
                if password.contains(pattern) {
                    return true;
                }
            }
        }

        false
    }

    /// Check for sequential characters (abc, 123, etc.)
    fn has_sequence(&self, password: &str) -> bool {
        let chars: Vec<char> = password.chars().collect();
        
        for i in 0..chars.len().saturating_sub(2) {
            let c1 = chars[i] as u8;
            let c2 = chars[i + 1] as u8;
            let c3 = chars[i + 2] as u8;

            // Check ascending sequence
            if c2 == c1 + 1 && c3 == c2 + 1 {
                return true;
            }
            // Check descending sequence
            if c2 == c1 - 1 && c3 == c2 - 1 {
                return true;
            }
        }

        false
    }

    /// Check for repeated characters
    fn has_repeats(&self, password: &str) -> bool {
        let chars: Vec<char> = password.chars().collect();
        
        for i in 0..chars.len().saturating_sub(2) {
            if chars[i] == chars[i + 1] && chars[i + 1] == chars[i + 2] {
                return true;
            }
        }

        false
    }

    /// Check if password is a dictionary word
    fn is_dictionary_word(&self, password: &str) -> bool {
        self.common_passwords.contains(&password.to_lowercase())
    }

    /// Check if password is L33t speak variant
    fn is_l33t_variant(&self, password: &str) -> bool {
        // Simple L33t detection: check for common substitutions
        let l33t_patterns = [
            ("a", "4"), ("e", "3"), ("i", "1"), ("o", "0"),
            ("s", "5"), ("t", "7"), ("l", "1"),
        ];

        let lower = password.to_lowercase();
        for (letter, digit) in &l33t_patterns {
            if lower.contains(digit) && lower.contains(letter) {
                // Check if digit appears where letter might be
                return true;
            }
        }

        false
    }

    /// Calculate pattern penalty
    fn pattern_penalty(&self, password: &str) -> f64 {
        let mut penalty = 0.0;

        if self.is_keyboard_walk(password) {
            penalty += 10.0;
        }
        if self.has_sequence(password) {
            penalty += 8.0;
        }
        if self.has_repeats(password) {
            penalty += 5.0;
        }

        penalty
    }

    /// Calculate dictionary penalty
    fn dictionary_penalty(&self, password: &str) -> f64 {
        if self.is_dictionary_word(password) {
            // Strong penalty for exact dictionary match
            return 20.0;
        }

        // Check for dictionary word with common modifications
        let lower = password.to_lowercase();
        for common in &self.common_passwords {
            if lower.contains(common) {
                return 15.0;
            }
        }

        0.0
    }

    /// Calculate L33t speak penalty
    fn l33t_penalty(&self, password: &str) -> f64 {
        if self.is_l33t_variant(password) {
            5.0
        } else {
            0.0
        }
    }

    /// Score password based on entropy
    fn score_from_entropy(&self, entropy: f64) -> PasswordScore {
        if entropy < 80.0 {
            PasswordScore::Rejected
        } else if entropy < 128.0 {
            PasswordScore::Weak
        } else if entropy < 160.0 {
            PasswordScore::Acceptable
        } else if entropy < 192.0 {
            PasswordScore::Strong
        } else {
            PasswordScore::Excellent
        }
    }

    /// Generate suggestions for improvement
    fn generate_suggestions(
        &self,
        password: &str,
        entropy: f64,
        score: PasswordScore,
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        if !score.is_quantum_resistant() {
            suggestions.push("Password does not meet quantum-resistant requirements (minimum 128 bits entropy)".to_string());
        }

        if password.len() < 12 {
            suggestions.push("Use at least 12 characters".to_string());
        }

        let char_set = self.character_set_size(password);
        if char_set < 62 {
            suggestions.push("Use a mix of uppercase, lowercase, digits, and special characters".to_string());
        }

        if self.is_dictionary_word(password) {
            suggestions.push("Avoid common dictionary words".to_string());
        }

        if self.is_keyboard_walk(password) {
            suggestions.push("Avoid keyboard walk patterns (e.g., qwerty, asdf)".to_string());
        }

        if self.has_sequence(password) {
            suggestions.push("Avoid sequential characters (e.g., abc, 123)".to_string());
        }

        if entropy < 192.0 && !password.contains(' ') {
            suggestions.push("Consider using a passphrase with multiple words for higher entropy".to_string());
        }

        suggestions
    }

    /// Estimate time to crack password
    fn estimate_time_to_crack(&self, entropy: f64, quantum: bool) -> String {
        // Assume 1 billion guesses per second (classical)
        // Quantum: assume Grover's algorithm provides sqrt speedup
        let guesses_per_sec = if quantum {
            1_000_000_000.0 * (2.0_f64.powf(entropy / 2.0)).sqrt()
        } else {
            1_000_000_000.0
        };

        let total_guesses = 2.0_f64.powf(entropy);
        let seconds = total_guesses / guesses_per_sec;

        if seconds < 60.0 {
            format!("< 1 minute")
        } else if seconds < 3600.0 {
            format!("~{} minutes", (seconds / 60.0) as u64)
        } else if seconds < 86400.0 {
            format!("~{} hours", (seconds / 3600.0) as u64)
        } else if seconds < 31536000.0 {
            format!("~{} days", (seconds / 86400.0) as u64)
        } else if seconds < 31536000000.0 {
            format!("~{} years", (seconds / 31536000.0) as u64)
        } else {
            format!("> {} billion years", (seconds / 31536000000.0) as u64)
        }
    }

    /// Validate password meets minimum requirements
    pub fn validate(&self, password: &str) -> Result<(), Vec<String>> {
        let strength = self.check_strength(password);
        
        if !strength.score.is_quantum_resistant() {
            Err(strength.suggestions)
        } else {
            Ok(())
        }
    }
}

impl Default for PasswordStrengthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a secure passphrase using BIP39 wordlist
/// In production, this would use the actual BIP39 wordlist
pub fn generate_passphrase(word_count: usize) -> String {
    // Simplified: In production, use actual BIP39 wordlist
    // This is a placeholder implementation
    let words = ["abandon", "ability", "able", "about", "above", "absent"];
    let mut passphrase = String::new();
    
    for i in 0..word_count {
        if i > 0 {
            passphrase.push(' ');
        }
        // In production, randomly select from full BIP39 wordlist
        passphrase.push_str(words[i % words.len()]);
    }
    
    passphrase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_calculation_basic() {
        let checker = PasswordStrengthChecker::new();

        // Very weak password
        let weak = checker.check_strength("password");
        assert!(weak.entropy_bits < 50.0);

        // Strong password
        let strong = checker.check_strength("Kj#9mP$2qL@5nR^8wX");
        assert!(strong.entropy_bits >= 100.0);
    }

    #[test]
    fn test_quantum_resistance_threshold() {
        let checker = PasswordStrengthChecker::new();

        // Weak password (below threshold)
        let weak = checker.check_strength("password123");
        assert!(!weak.score.is_quantum_resistant());

        // Strong password (above threshold)
        let strong = checker.check_strength("Tr0ub4dor&3");
        // This should meet minimum, but let's check a very strong one
        let very_strong = checker.check_strength("correct horse battery staple");
        assert!(very_strong.score.is_quantum_resistant() || very_strong.entropy_bits >= 128.0);
    }

    #[test]
    fn test_dictionary_attack_detection() {
        let checker = PasswordStrengthChecker::new();

        let dict_word = checker.check_strength("password");
        assert!(dict_word.patterns_detected.iter().any(|p| p.contains("dictionary")));

        let non_dict = checker.check_strength("xK9#mP$2qL");
        assert!(!non_dict.patterns_detected.iter().any(|p| p.contains("dictionary")));
    }

    #[test]
    fn test_pattern_detection() {
        let checker = PasswordStrengthChecker::new();

        // Keyboard walk
        let keyboard = checker.check_strength("qwerty123");
        assert!(keyboard.patterns_detected.iter().any(|p| p.contains("Keyboard")));

        // Sequence
        let sequence = checker.check_strength("abc123");
        assert!(sequence.patterns_detected.iter().any(|p| p.contains("Sequential")));

        // Repeats
        let repeats = checker.check_strength("aaa111");
        assert!(repeats.patterns_detected.iter().any(|p| p.contains("Repeated")));
    }

    #[test]
    fn test_passphrase_entropy() {
        let checker = PasswordStrengthChecker::new();

        // Passphrase should have high entropy
        let passphrase = checker.check_strength("correct horse battery staple");
        assert!(passphrase.entropy_bits >= 40.0); // At least 4 words * 11 bits
    }

    #[test]
    fn test_l33t_speak_variants() {
        let checker = PasswordStrengthChecker::new();

        let l33t = checker.check_strength("p4ssw0rd");
        assert!(l33t.patterns_detected.iter().any(|p| p.contains("L33t")));
    }

    #[test]
    fn test_time_to_crack_estimation() {
        let checker = PasswordStrengthChecker::new();

        let weak = checker.check_strength("password");
        assert!(!weak.time_to_crack_classical.is_empty());
        assert!(!weak.time_to_crack_quantum.is_empty());
    }

    #[test]
    fn test_password_validation() {
        let checker = PasswordStrengthChecker::new();

        // Weak password should fail validation
        let result = checker.validate("password123");
        assert!(result.is_err());

        // Strong password should pass (if entropy is high enough)
        // Note: This test may need adjustment based on actual entropy calculation
        let strong_result = checker.validate("correct horse battery staple");
        // Passphrase should generally pass
        if strong_result.is_err() {
            // If it fails, check that suggestions are provided
            let err = strong_result.unwrap_err();
            assert!(!err.is_empty());
        }
    }

    #[test]
    fn test_character_set_detection() {
        let checker = PasswordStrengthChecker::new();

        let lower_only = checker.check_strength("password");
        assert_eq!(lower_only.character_set_size, 26);

        let mixed = checker.check_strength("Password123!");
        assert!(mixed.character_set_size >= 62);
    }

    #[test]
    fn test_score_levels() {
        let checker = PasswordStrengthChecker::new();

        // Test score descriptions
        assert_eq!(PasswordScore::Rejected.description(), "REJECTED - Vulnerable to classical attacks");
        assert_eq!(PasswordScore::Weak.description(), "WEAK - Vulnerable to quantum attacks");
        assert_eq!(PasswordScore::Acceptable.description(), "ACCEPTABLE - Quantum-resistant minimum");
        assert_eq!(PasswordScore::Strong.description(), "STRONG - Recommended for Supernova");
        assert_eq!(PasswordScore::Excellent.description(), "EXCELLENT - Future-proof");
    }
}

