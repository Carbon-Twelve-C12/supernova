use crate::api::types::LogEntry;
use chrono::Utc;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use regex::Regex;
use lazy_static::lazy_static;

lazy_static::lazy_static! {
    static ref LOG_BUFFER: Arc<Mutex<VecDeque<LogEntry>>> = Arc::new(Mutex::new(VecDeque::with_capacity(10000)));
    static ref LOG_REDACTOR: Arc<Mutex<LogRedactor>> = Arc::new(Mutex::new(LogRedactor::new()));
}

/// Redaction level for sensitive data filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionLevel {
    /// Full redaction - completely mask sensitive data
    Full,
    /// Partial redaction - show first/last few characters
    Partial,
    /// No redaction - log everything (use with extreme caution)
    None,
}

/// Sensitive data redactor for log messages
/// Prevents accidental exposure of private keys, seeds, passwords, and other sensitive information
pub struct LogRedactor {
    /// Hex private key pattern (64+ hex characters)
    hex_key_pattern: Regex,
    /// Base58 private key pattern
    base58_pattern: Regex,
    /// Seed phrase pattern (12 or 24 words)
    seed_phrase_pattern: Regex,
    /// Password pattern (common password indicators)
    password_pattern: Regex,
    /// API key pattern
    api_key_pattern: Regex,
    /// Current redaction level
    redaction_level: RedactionLevel,
}

impl LogRedactor {
    /// Create a new log redactor with default patterns
    pub fn new() -> Self {
        Self {
            hex_key_pattern: Regex::new(r"(?i)(?:private[_\s]?key|secret[_\s]?key|privkey|sk)[\s:=]+([0-9a-f]{64,})").unwrap(),
            base58_pattern: Regex::new(r"(?i)(?:private[_\s]?key|secret[_\s]?key|privkey|sk)[\s:=]+([123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{50,})").unwrap(),
            seed_phrase_pattern: Regex::new(r"(?i)(?:seed|mnemonic|phrase)[\s:=]+((?:[a-z]+\s+){11,23}[a-z]+)").unwrap(),
            password_pattern: Regex::new(r"(?i)(?:password|passwd|pwd)[\s:=]+([^\s]+)").unwrap(),
            api_key_pattern: Regex::new(r"(?i)(?:api[_\s]?key|apikey|token)[\s:=]+([a-zA-Z0-9_\-]{20,})").unwrap(),
            redaction_level: RedactionLevel::Full,
        }
    }

    /// Set the redaction level
    pub fn set_redaction_level(&mut self, level: RedactionLevel) {
        self.redaction_level = level;
    }

    /// Redact sensitive data from a log message
    pub fn redact(&self, message: &str) -> String {
        if self.redaction_level == RedactionLevel::None {
            return message.to_string();
        }

        let mut redacted = message.to_string();

        // Redact hex private keys
        redacted = self.hex_key_pattern.replace_all(&redacted, |caps: &regex::Captures| {
            let prefix = caps.get(0).map(|m| {
                let full_match = m.as_str();
                let key_match = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                if let Some(pos) = full_match.find(key_match) {
                    &full_match[..pos]
                } else {
                    full_match
                }
            }).unwrap_or("");
            format!("{}[REDACTED]", prefix)
        }).to_string();

        // Redact base58 private keys
        redacted = self.base58_pattern.replace_all(&redacted, |caps: &regex::Captures| {
            let prefix = caps.get(0).map(|m| {
                let full_match = m.as_str();
                let key_match = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                if let Some(pos) = full_match.find(key_match) {
                    &full_match[..pos]
                } else {
                    full_match
                }
            }).unwrap_or("");
            format!("{}[REDACTED]", prefix)
        }).to_string();

        // Redact seed phrases
        redacted = self.seed_phrase_pattern.replace_all(&redacted, |caps: &regex::Captures| {
            if let Some(seed_match) = caps.get(1) {
                let seed = seed_match.as_str();
                if self.redaction_level == RedactionLevel::Partial {
                    let words: Vec<&str> = seed.split_whitespace().collect();
                    if words.len() >= 4 {
                        format!("seed=[{}...{}] [{} words]", words[0], words[words.len() - 1], words.len())
                    } else {
                        "seed=[REDACTED]".to_string()
                    }
                } else {
                    let prefix = caps.get(0).map(|m| {
                        let full_match = m.as_str();
                        if let Some(pos) = full_match.find(seed) {
                            &full_match[..pos]
                        } else {
                            full_match
                        }
                    }).unwrap_or("");
                    format!("{}[REDACTED]", prefix)
                }
            } else {
                "[REDACTED]".to_string()
            }
        }).to_string();

        // Redact passwords
        redacted = self.password_pattern.replace_all(&redacted, |caps: &regex::Captures| {
            if let Some(pwd_match) = caps.get(1) {
                let pwd = pwd_match.as_str();
                if self.redaction_level == RedactionLevel::Partial && pwd.len() > 4 {
                    format!("password={}...{}", &pwd[..2], &pwd[pwd.len() - 2..])
                } else {
                    let prefix = caps.get(0).map(|m| {
                        let full_match = m.as_str();
                        if let Some(pos) = full_match.find(pwd) {
                            &full_match[..pos]
                        } else {
                            full_match
                        }
                    }).unwrap_or("");
                    format!("{}[REDACTED]", prefix)
                }
            } else {
                "[REDACTED]".to_string()
            }
        }).to_string();

        // Redact API keys
        redacted = self.api_key_pattern.replace_all(&redacted, |caps: &regex::Captures| {
            if let Some(key_match) = caps.get(1) {
                let key = key_match.as_str();
                if self.redaction_level == RedactionLevel::Partial && key.len() > 8 {
                    format!("api_key={}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    let prefix = caps.get(0).map(|m| {
                        let full_match = m.as_str();
                        if let Some(pos) = full_match.find(key) {
                            &full_match[..pos]
                        } else {
                            full_match
                        }
                    }).unwrap_or("");
                    format!("{}[REDACTED]", prefix)
                }
            } else {
                "[REDACTED]".to_string()
            }
        }).to_string();

        // Additional pattern: Hex strings that look like private keys (standalone)
        redacted = Self::redact_standalone_hex_keys(&redacted, self.redaction_level);

        // Additional pattern: Base58 strings that look like keys (standalone)
        redacted = Self::redact_standalone_base58_keys(&redacted, self.redaction_level);

        redacted
    }

    /// Redact standalone hex keys (64+ hex characters without context)
    fn redact_standalone_hex_keys(text: &str, level: RedactionLevel) -> String {
        lazy_static! {
            static ref HEX_PATTERN: Regex = Regex::new(r"\b([0-9a-f]{64,})\b").unwrap();
        }

        if level == RedactionLevel::None {
            return text.to_string();
        }

        HEX_PATTERN.replace_all(text, |caps: &regex::Captures| {
            let hex = &caps[1];
            if hex.len() >= 128 {
                // Very long hex strings are likely keys
                if level == RedactionLevel::Partial {
                    format!("{}...{}", &hex[..8], &hex[hex.len() - 8..])
                } else {
                    "[REDACTED]".to_string()
                }
            } else if hex.len() == 64 {
                // Exactly 64 hex chars could be a private key
                if level == RedactionLevel::Partial {
                    format!("{}...{}", &hex[..8], &hex[hex.len() - 8..])
                } else {
                    "[REDACTED]".to_string()
                }
            } else {
                // Keep shorter hex strings as-is (likely hashes, addresses)
                hex.to_string()
            }
        }).to_string()
    }

    /// Redact standalone base58 keys (50+ base58 characters)
    fn redact_standalone_base58_keys(text: &str, level: RedactionLevel) -> String {
        lazy_static! {
            static ref BASE58_PATTERN: Regex = Regex::new(r"\b([123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{50,})\b").unwrap();
        }

        if level == RedactionLevel::None {
            return text.to_string();
        }

        BASE58_PATTERN.replace_all(text, |caps: &regex::Captures| {
            let base58 = &caps[1];
            if base58.len() >= 50 {
                if level == RedactionLevel::Partial {
                    format!("{}...{}", &base58[..8], &base58[base58.len() - 8..])
                } else {
                    "[REDACTED]".to_string()
                }
            } else {
                base58.to_string()
            }
        }).to_string()
    }
}

impl Default for LogRedactor {
    fn default() -> Self {
        Self::new()
    }
}

/// Get recent logs based on filters
pub fn get_recent_logs(
    level: &str,
    component: Option<&str>,
    limit: usize,
    offset: usize,
) -> Vec<LogEntry> {
    let buffer = match LOG_BUFFER.lock() {
        Ok(b) => b,
        Err(_) => return Vec::new(), // Return empty on lock poisoned
    };

    buffer
        .iter()
        .filter(|log| {
            // Filter by level
            if !level.is_empty() && log.level.to_lowercase() != level.to_lowercase() {
                return false;
            }

            // Filter by component if specified
            if let Some(comp) = component {
                if !log.component.contains(comp) {
                    return false;
                }
            }

            true
        })
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

/// Add a log entry to the buffer
/// Automatically redacts sensitive data before storing
pub fn add_log_entry(level: &str, component: &str, message: String) {
    let mut buffer = match LOG_BUFFER.lock() {
        Ok(b) => b,
        Err(_) => return, // Skip logging on lock poisoned
    };

    // Remove oldest entries if buffer is full
    if buffer.len() >= 10000 {
        buffer.pop_front();
    }

    // Redact sensitive data before storing
    let redacted_message = {
        let redactor = match LOG_REDACTOR.lock() {
            Ok(r) => r,
            Err(_) => return, // Skip logging on lock poisoned
        };
        redactor.redact(&message)
    };

    buffer.push_back(LogEntry {
        timestamp: Utc::now().timestamp() as u64,
        level: level.to_string(),
        component: component.to_string(),
        message: redacted_message,
        context: None,
    });
}

/// Set the redaction level for log messages
pub fn set_redaction_level(level: RedactionLevel) {
    if let Ok(mut redactor) = LOG_REDACTOR.lock() {
        redactor.set_redaction_level(level);
    }
}

/// Initialize the logging system
pub fn init_logging() {
    // Set up tracing subscriber that also writes to our buffer
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true);

    let buffer_layer = BufferLayer;

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(buffer_layer)
        .init();
}

/// Custom tracing layer that writes to our log buffer
struct BufferLayer;

impl<S> tracing_subscriber::Layer<S> for BufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing_subscriber::field::Visit;

        struct Visitor {
            message: String,
        }

        impl Visit for Visitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                }
            }
        }

        let mut visitor = Visitor {
            message: String::new(),
        };

        event.record(&mut visitor);

        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARN",
            tracing::Level::INFO => "INFO",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::TRACE => "TRACE",
        };

        let component = event.metadata().target();

        // Message is already redacted by add_log_entry
        add_log_entry(level, component, visitor.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_key_redaction() {
        let redactor = LogRedactor::new();

        // Test hex private key redaction
        let message = "Private key: a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890";
        let redacted = redactor.redact(message);
        assert!(
            !redacted.contains("a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890"),
            "Private key should be redacted"
        );
        assert!(
            redacted.contains("[REDACTED]"),
            "Redacted message should contain [REDACTED] marker"
        );

        // Test base58 private key redaction
        let message2 = "Secret key: 5KJvsngHeMoiSDxQwU5JjqZ3q9Z8q7q6q5q4q3q2q1q0q9q8q7q6q5q4q3q2q1q0";
        let redacted2 = redactor.redact(message2);
        assert!(
            !redacted2.contains("5KJvsngHeMoiSDxQwU5JjqZ3q9Z8q7q6q5q4q3q2q1q0q9q8q7q6q5q4q3q2q1q0"),
            "Base58 private key should be redacted"
        );
    }

    #[test]
    fn test_seed_phrase_redaction() {
        let redactor = LogRedactor::new();

        // Test seed phrase redaction (full)
        let message = "Seed phrase: abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let redacted = redactor.redact(message);
        assert!(
            !redacted.contains("abandon abandon abandon"),
            "Seed phrase should be redacted"
        );
        assert!(
            redacted.contains("[REDACTED]"),
            "Redacted message should contain [REDACTED] marker"
        );

        // Test seed phrase redaction (partial)
        let mut redactor_partial = LogRedactor::new();
        redactor_partial.set_redaction_level(RedactionLevel::Partial);
        let message2 = "Mnemonic: word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12";
        let redacted2 = redactor_partial.redact(message2);
        assert!(
            !redacted2.contains("word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12"),
            "Seed phrase should be partially redacted"
        );
        assert!(
            redacted2.contains("word1") || redacted2.contains("word12"),
            "Partial redaction should show first/last word"
        );
    }

    #[test]
    fn test_password_redaction() {
        let redactor = LogRedactor::new();

        // Test password redaction (full)
        let message = "Password: mySecretPassword123";
        let redacted = redactor.redact(message);
        assert!(
            !redacted.contains("mySecretPassword123"),
            "Password should be redacted"
        );
        assert!(
            redacted.contains("[REDACTED]"),
            "Redacted message should contain [REDACTED] marker"
        );

        // Test password redaction (partial)
        let mut redactor_partial = LogRedactor::new();
        redactor_partial.set_redaction_level(RedactionLevel::Partial);
        let message2 = "password=superSecret123";
        let redacted2 = redactor_partial.redact(message2);
        assert!(
            !redacted2.contains("superSecret123"),
            "Password should be partially redacted"
        );
        assert!(
            redacted2.contains("su") || redacted2.contains("23"),
            "Partial redaction should show first/last chars"
        );
    }

    #[test]
    fn test_api_key_redaction() {
        let redactor = LogRedactor::new();

        // Test API key redaction (full)
        let message = "API key: test_api_key_1234567890abcdefghijklmnopqrstuvwxyz";
        let redacted = redactor.redact(message);
        assert!(
            !redacted.contains("test_api_key_1234567890abcdefghijklmnopqrstuvwxyz"),
            "API key should be redacted"
        );
        assert!(
            redacted.contains("[REDACTED]"),
            "Redacted message should contain [REDACTED] marker"
        );

        // Test API key redaction (partial)
        let mut redactor_partial = LogRedactor::new();
        redactor_partial.set_redaction_level(RedactionLevel::Partial);
        let message2 = "apikey=abcdef1234567890";
        let redacted2 = redactor_partial.redact(message2);
        assert!(
            !redacted2.contains("abcdef1234567890"),
            "API key should be partially redacted"
        );
        assert!(
            redacted2.contains("abcd") || redacted2.contains("7890"),
            "Partial redaction should show first/last chars"
        );
    }

    #[test]
    fn test_partial_address_redaction() {
        let redactor = LogRedactor::new();

        // Test that shorter hex strings (addresses) are not redacted by standalone pattern
        let message = "Address: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let redacted = redactor.redact(message);
        // Addresses should not be redacted as they're public (unless they match key patterns)
        // The important thing is that private keys are redacted

        // Test that long hex strings (likely keys) are redacted
        let message2 = "Key: a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let redacted2 = redactor.redact(message2);
        assert!(
            redacted2.contains("[REDACTED]"),
            "Very long hex strings should be redacted"
        );
    }

    #[test]
    fn test_redaction_performance() {
        let redactor = LogRedactor::new();

        // Create a message with multiple sensitive patterns
        let message = format!(
            "Private key: {}\nPassword: {}\nAPI key: {}\nSeed: {}",
            "a".repeat(64),
            "password123",
            "test_api_key_".to_string() + &"x".repeat(32),
            "word ".repeat(11) + "word12"
        );

        // Measure redaction time (should be fast)
        let start = std::time::Instant::now();
        let _redacted = redactor.redact(&message);
        let duration = start.elapsed();

        // Redaction should complete in reasonable time (< 10ms for this message)
        assert!(
            duration.as_millis() < 10,
            "Redaction should be fast: {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_redaction_level_none() {
        let mut redactor = LogRedactor::new();
        redactor.set_redaction_level(RedactionLevel::None);

        let message = "Private key: a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890";
        let redacted = redactor.redact(message);

        // With None level, message should be unchanged
        assert!(
            redacted.contains("a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890"),
            "With None level, sensitive data should not be redacted"
        );
    }

    #[test]
    fn test_multiple_patterns_in_message() {
        let redactor = LogRedactor::new();

        let message = "Private key: a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890 Password: secret123 API key: test_api_key_1234567890abcdefghijklmnopqrstuvwxyz";
        let redacted = redactor.redact(message);

        // All patterns should be redacted
        assert!(
            !redacted.contains("a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890"),
            "Private key should be redacted"
        );
        assert!(
            !redacted.contains("secret123"),
            "Password should be redacted"
        );
        assert!(
            !redacted.contains("test_api_key_1234567890abcdefghijklmnopqrstuvwxyz"),
            "API key should be redacted"
        );
        assert!(
            redacted.matches("[REDACTED]").count() >= 3,
            "Should have multiple redactions"
        );
    }

    #[test]
    fn test_standalone_hex_key_redaction() {
        let redactor = LogRedactor::new();

        // Test standalone 64-char hex (likely private key)
        let message = "Key a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890 found";
        let redacted = redactor.redact(message);
        assert!(
            redacted.contains("[REDACTED]"),
            "Standalone 64-char hex should be redacted"
        );
    }
}
