//! Wallet Backup Warnings and Verification
//!
//! Implements prominent backup warnings and verification system to ensure
//! users properly back up their wallet seed phrases.
//!
//! Features:
//! - Backup warning on wallet creation
//! - Seed phrase verification flow
//! - Backup status tracking
//! - Periodic backup reminders
//! - Backup verification command

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Backup status for a wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupStatus {
    /// Wallet not backed up
    NotBackedUp,
    /// Backup verified (user confirmed seed phrase)
    Verified,
    /// Backup acknowledged but not verified
    Acknowledged,
}

/// Backup metadata stored in wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Backup status
    pub status: BackupStatus,
    /// When wallet was created
    pub created_at: DateTime<Utc>,
    /// When backup was last verified
    pub verified_at: Option<DateTime<Utc>>,
    /// When backup was last acknowledged
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Number of verification attempts
    pub verification_attempts: u32,
}

impl BackupMetadata {
    /// Create new backup metadata for a new wallet
    pub fn new() -> Self {
        Self {
            status: BackupStatus::NotBackedUp,
            created_at: Utc::now(),
            verified_at: None,
            acknowledged_at: None,
            verification_attempts: 0,
        }
    }

    /// Mark backup as acknowledged
    pub fn acknowledge(&mut self) {
        self.status = BackupStatus::Acknowledged;
        self.acknowledged_at = Some(Utc::now());
    }

    /// Mark backup as verified
    pub fn verify(&mut self) {
        self.status = BackupStatus::Verified;
        self.verified_at = Some(Utc::now());
        self.verification_attempts += 1;
    }

    /// Check if backup reminder is needed (7 days since creation)
    pub fn needs_reminder(&self) -> bool {
        if self.status == BackupStatus::Verified {
            return false;
        }

        let days_since_creation = Utc::now()
            .signed_duration_since(self.created_at)
            .num_days();

        days_since_creation >= 7
    }

    /// Check if backup is overdue (30 days since creation)
    pub fn is_overdue(&self) -> bool {
        if self.status == BackupStatus::Verified {
            return false;
        }

        let days_since_creation = Utc::now()
            .signed_duration_since(self.created_at)
            .num_days();

        days_since_creation >= 30
    }
}

impl Default for BackupMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Backup warning display
pub struct BackupWarning;

impl BackupWarning {
    /// Display critical backup warning
    pub fn display_warning() {
        println!("\n{}", "=".repeat(80));
        println!("{}", "⚠️  CRITICAL SECURITY WARNING ⚠️".repeat(1));
        println!("{}", "=".repeat(80));
        println!();
        println!("Your wallet seed phrase is the ONLY way to recover your funds.");
        println!("If you lose your seed phrase, your funds will be PERMANENTLY LOST.");
        println!();
        println!("IMPORTANT:");
        println!("  • Write down your seed phrase on paper");
        println!("  • Store it in a secure, offline location");
        println!("  • NEVER share your seed phrase with anyone");
        println!("  • NEVER store it digitally (screenshots, cloud storage, etc.)");
        println!("  • Consider using a hardware wallet for large amounts");
        println!();
        println!("{}", "=".repeat(80));
        println!();
    }

    /// Display seed phrase with warning
    pub fn display_seed_phrase(seed_phrase: &str) {
        Self::display_warning();
        
        println!("Your seed phrase (12 words):");
        println!("{}", "-".repeat(80));
        println!("{}", seed_phrase);
        println!("{}", "-".repeat(80));
        println!();
        println!("⚠️  Write this down NOW before continuing! ⚠️");
        println!();
    }

    /// Display backup reminder
    pub fn display_reminder(days_since_creation: i64) {
        println!("\n{}", "=".repeat(80));
        println!("⚠️  BACKUP REMINDER ⚠️");
        println!("{}", "=".repeat(80));
        println!();
        println!("Your wallet was created {} days ago and has not been verified.", days_since_creation);
        println!("Please verify your backup to ensure you can recover your wallet.");
        println!();
        println!("Run: wallet verify-backup");
        println!();
    }

    /// Display overdue backup warning
    pub fn display_overdue_warning(days_since_creation: i64) {
        println!("\n{}", "=".repeat(80));
        println!("🚨 CRITICAL: BACKUP OVERDUE 🚨");
        println!("{}", "=".repeat(80));
        println!();
        println!("Your wallet was created {} days ago and backup has NOT been verified!", days_since_creation);
        println!("Your funds are at RISK if you lose access to this device.");
        println!();
        println!("ACTION REQUIRED:");
        println!("  1. Locate your seed phrase backup");
        println!("  2. Run: wallet verify-backup");
        println!("  3. Verify you can restore your wallet from the seed phrase");
        println!();
        println!("{}", "=".repeat(80));
        println!();
    }
}

/// Seed phrase verifier
pub struct SeedPhraseVerifier;

impl SeedPhraseVerifier {
    /// Verify seed phrase by asking user to re-enter random words
    /// 
    /// # Arguments
    /// * `seed_phrase` - The full seed phrase to verify
    /// * `skip_check` - Skip verification (for automated testing only)
    /// 
    /// # Returns
    /// * `Ok(true)` - Verification successful
    /// * `Ok(false)` - User declined verification
    /// * `Err(String)` - Verification failed
    pub fn verify_interactive(
        seed_phrase: &str,
        skip_check: bool,
    ) -> Result<bool, String> {
        if skip_check {
            println!("⚠️  Skipping backup verification (testing mode)");
            return Ok(true);
        }

        let words: Vec<&str> = seed_phrase.split_whitespace().collect();
        
        if words.len() < 12 {
            return Err("Invalid seed phrase length".to_string());
        }

        println!("\n{}", "=".repeat(80));
        println!("SEED PHRASE VERIFICATION");
        println!("{}", "=".repeat(80));
        println!();
        println!("To verify you have correctly backed up your seed phrase,");
        println!("please enter the following words from your backup:");
        println!();

        // Select 3 random word positions
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let positions: Vec<usize> = {
            let mut pos = Vec::new();
            while pos.len() < 3 {
                let p = rng.gen_range(0..words.len());
                if !pos.contains(&p) {
                    pos.push(p);
                }
            }
            pos.sort();
            pos
        };

        // Ask for each word
        for (i, &pos) in positions.iter().enumerate() {
            print!("Word #{} (position {}): ", i + 1, pos + 1);
            use std::io::{self, Write};
            io::stdout().flush().map_err(|e| format!("IO error: {}", e))?;
            
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("IO error: {}", e))?;
            
            let input_word = input.trim().to_lowercase();
            let expected_word = words[pos].to_lowercase();

            if input_word != expected_word {
                return Err(format!(
                    "Verification failed: Word #{} incorrect. Expected '{}', got '{}'",
                    i + 1, words[pos], input_word
                ));
            }
        }

        println!();
        println!("✓ Seed phrase verification successful!");
        println!("Your backup is verified and you can recover your wallet.");
        println!();

        Ok(true)
    }

    /// Verify seed phrase programmatically (for testing)
    pub fn verify_programmatic(
        seed_phrase: &str,
        expected_words: &[String],
        positions: &[usize],
    ) -> Result<bool, String> {
        let words: Vec<&str> = seed_phrase.split_whitespace().collect();

        if positions.len() != expected_words.len() {
            return Err("Position and word count mismatch".to_string());
        }

        for (i, &pos) in positions.iter().enumerate() {
            if pos >= words.len() {
                return Err(format!("Position {} out of range", pos));
            }

            if words[pos].to_lowercase() != expected_words[i].to_lowercase() {
                return Err(format!(
                    "Word at position {} incorrect: expected '{}', got '{}'",
                    pos + 1, expected_words[i], words[pos]
                ));
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_warning_on_creation() {
        let metadata = BackupMetadata::new();
        assert_eq!(metadata.status, BackupStatus::NotBackedUp);
        assert!(metadata.verified_at.is_none());
    }

    #[test]
    fn test_seed_phrase_verification() {
        let seed_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident";
        let expected_words = vec!["about".to_string(), "above".to_string(), "access".to_string()];
        let positions = vec![3, 4, 10];

        let result = SeedPhraseVerifier::verify_programmatic(
            seed_phrase,
            &expected_words,
            &positions,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_backup_status_tracking() {
        let mut metadata = BackupMetadata::new();
        
        // Initially not backed up
        assert_eq!(metadata.status, BackupStatus::NotBackedUp);
        
        // Acknowledge backup
        metadata.acknowledge();
        assert_eq!(metadata.status, BackupStatus::Acknowledged);
        assert!(metadata.acknowledged_at.is_some());
        
        // Verify backup
        metadata.verify();
        assert_eq!(metadata.status, BackupStatus::Verified);
        assert!(metadata.verified_at.is_some());
        assert_eq!(metadata.verification_attempts, 1);
    }

    #[test]
    fn test_backup_reminder_after_period() {
        let mut metadata = BackupMetadata::new();
        
        // Set creation time to 8 days ago
        metadata.created_at = Utc::now() - chrono::Duration::days(8);
        
        // Should need reminder
        assert!(metadata.needs_reminder());
        
        // Verify backup
        metadata.verify();
        
        // Should not need reminder after verification
        assert!(!metadata.needs_reminder());
    }

    #[test]
    fn test_backup_verification_command() {
        let seed_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident";
        
        // Test successful verification
        let result = SeedPhraseVerifier::verify_programmatic(
            seed_phrase,
            &vec!["about".to_string(), "above".to_string()],
            &vec![3, 4],
        );
        
        assert!(result.is_ok());
        
        // Test failed verification
        let result = SeedPhraseVerifier::verify_programmatic(
            seed_phrase,
            &vec!["wrong".to_string(), "above".to_string()],
            &vec![3, 4],
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_overdue_backup() {
        let mut metadata = BackupMetadata::new();
        
        // Set creation time to 31 days ago
        metadata.created_at = Utc::now() - chrono::Duration::days(31);
        
        // Should be overdue
        assert!(metadata.is_overdue());
        
        // Verify backup
        metadata.verify();
        
        // Should not be overdue after verification
        assert!(!metadata.is_overdue());
    }
}

