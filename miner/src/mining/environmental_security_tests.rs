use super::environmental_verification::{EnvironmentalVerifier, RECCertificate, EfficiencyAudit, VerificationError};
use super::reward::EnvironmentalProfile;

#[cfg(test)]
mod environmental_security_tests {
    use super::*;
    
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    
    #[tokio::test]
    async fn test_certificate_replay_attack_prevention() {
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        // Create a valid certificate
        let cert = RECCertificate {
            certificate_id: "CERT-001".to_string(),
            issuer: "TrustedIssuer".to_string(),
            coverage_mwh: 100.0,
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 3600,
            verified: false,
        };
        
        // Register the certificate
        verifier.register_rec_certificate(cert.clone()).await;
        
        // First miner uses the certificate
        let result1 = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![cert.clone()],
            None,
        ).await;
        assert!(result1.is_ok());
        
        // Second miner tries to use the same certificate (should fail due to consumption tracking)
        let result2 = verifier.verify_miner_profile(
            "miner2".to_string(),
            EnvironmentalProfile::default(),
            vec![cert.clone()],
            None,
        ).await;
        
        // Verify that certificate reuse is prevented
        assert!(result2.is_err());
        match result2 {
            Err(VerificationError::CertificateAlreadyConsumed(id)) => {
                assert_eq!(id, "CERT-001");
            }
            _ => panic!("Expected CertificateAlreadyConsumed error"),
        }
    }
    
    #[tokio::test]
    async fn test_expired_certificate_rejection() {
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        // Create an expired certificate
        let expired_cert = RECCertificate {
            certificate_id: "EXPIRED-001".to_string(),
            issuer: "TrustedIssuer".to_string(),
            coverage_mwh: 100.0,
            valid_from: current_timestamp() - 7200,
            valid_until: current_timestamp() - 3600, // Expired
            verified: false,
        };
        
        verifier.register_rec_certificate(expired_cert.clone()).await;
        
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![expired_cert],
            None,
        ).await;
        
        // Should succeed but with 0% renewable
        assert!(result.is_ok());
        assert_eq!(result.unwrap().environmental_profile.renewable_percentage, 0.0);
    }
    
    #[tokio::test]
    async fn test_untrusted_issuer_rejection() {
        let verifier = EnvironmentalVerifier::new();
        // Don't register the issuer as trusted
        
        let cert = RECCertificate {
            certificate_id: "UNTRUSTED-001".to_string(),
            issuer: "UntrustedIssuer".to_string(),
            coverage_mwh: 1000.0, // Large amount
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 3600,
            verified: false,
        };
        
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![cert],
            None,
        ).await;
        
        // Should succeed but with 0% renewable (untrusted cert ignored)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().environmental_profile.renewable_percentage, 0.0);
    }
    
    #[tokio::test]
    async fn test_certificate_tampering_detection() {
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        // Register a certificate
        let original_cert = RECCertificate {
            certificate_id: "TAMPER-001".to_string(),
            issuer: "TrustedIssuer".to_string(),
            coverage_mwh: 50.0,
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 3600,
            verified: false,
        };
        verifier.register_rec_certificate(original_cert.clone()).await;
        
        // Try to use a tampered version with higher coverage
        let tampered_cert = RECCertificate {
            certificate_id: "TAMPER-001".to_string(),
            issuer: "TrustedIssuer".to_string(),
            coverage_mwh: 500.0, // Tampered value
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 3600,
            verified: false,
        };
        
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![tampered_cert],
            None,
        ).await;
        
        // Should succeed but with 0% renewable (tampered cert rejected)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().environmental_profile.renewable_percentage, 0.0);
    }
    
    #[tokio::test]
    async fn test_efficiency_audit_manipulation() {
        let verifier = EnvironmentalVerifier::new();
        
        // Test with unrealistic efficiency values
        let fake_audit = EfficiencyAudit {
            auditor: "FakeAuditor".to_string(),
            hash_rate_per_watt: 1000000.0, // Unrealistically high
            cooling_efficiency: 2.0, // Impossible (>100%)
            overall_pue: 0.5, // Impossible (< 1.0)
            audit_timestamp: current_timestamp() - 3600,
        };
        
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![],
            Some(fake_audit),
        ).await;
        
        // Should succeed but efficiency score should be capped
        assert!(result.is_ok());
        let profile = result.unwrap();
        assert!(profile.environmental_profile.efficiency_score <= 1.0);
    }
    
    #[tokio::test]
    async fn test_profile_expiry_enforcement() {
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        let cert = RECCertificate {
            certificate_id: "CERT-002".to_string(),
            issuer: "TrustedIssuer".to_string(),
            coverage_mwh: 100.0,
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 3600,
            verified: false,
        };
        verifier.register_rec_certificate(cert.clone()).await;
        
        // Verify profile
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            vec![cert],
            None,
        ).await;
        assert!(result.is_ok());
        
        // Check that profile is returned
        let profile1 = verifier.get_verified_profile("miner1").await;
        assert!(profile1.is_some());
        
        // Simulate time passing (would need to mock time in production)
        // For now, just verify the expiry is set correctly
        let verified_profile = result.unwrap();
        assert!(verified_profile.verification_expiry > current_timestamp());
        assert_eq!(
            verified_profile.verification_expiry - verified_profile.verification_timestamp,
            30 * 24 * 3600 // 30 days
        );
    }
    
    #[tokio::test]
    async fn test_concurrent_verification_safety() {
        use tokio::task;
        
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        // Register multiple certificates
        for i in 0..10 {
            let cert = RECCertificate {
                certificate_id: format!("CONCURRENT-{}", i),
                issuer: "TrustedIssuer".to_string(),
                coverage_mwh: 10.0,
                valid_from: current_timestamp() - 3600,
                valid_until: current_timestamp() + 3600,
                verified: false,
            };
            verifier.register_rec_certificate(cert).await;
        }
        
        // Spawn multiple verification tasks concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let verifier_clone = verifier.clone();
            let handle = task::spawn(async move {
                let cert = RECCertificate {
                    certificate_id: format!("CONCURRENT-{}", i),
                    issuer: "TrustedIssuer".to_string(),
                    coverage_mwh: 10.0,
                    valid_from: current_timestamp() - 3600,
                    valid_until: current_timestamp() + 3600,
                    verified: false,
                };
                
                verifier_clone.verify_miner_profile(
                    format!("miner{}", i),
                    EnvironmentalProfile::default(),
                    vec![cert],
                    None,
                ).await
            });
            handles.push(handle);
        }
        
        // All verifications should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_maximum_certificate_stacking() {
        let verifier = EnvironmentalVerifier::new();
        verifier.register_trusted_issuer("TrustedIssuer".to_string()).await;
        
        // Create many certificates to try to stack bonuses
        let mut certs = vec![];
        for i in 0..100 {
            let cert = RECCertificate {
                certificate_id: format!("STACK-{}", i),
                issuer: "TrustedIssuer".to_string(),
                coverage_mwh: 10.0,
                valid_from: current_timestamp() - 3600,
                valid_until: current_timestamp() + 3600,
                verified: false,
            };
            verifier.register_rec_certificate(cert.clone()).await;
            certs.push(cert);
        }
        
        let result = verifier.verify_miner_profile(
            "miner1".to_string(),
            EnvironmentalProfile::default(),
            certs,
            None,
        ).await;
        
        // Should succeed but renewable percentage should be capped at 100%
        assert!(result.is_ok());
        let profile = result.unwrap();
        assert!(profile.environmental_profile.renewable_percentage <= 1.0);
        assert_eq!(profile.environmental_profile.renewable_percentage, 1.0); // 1000 MWh > 100 MWh threshold
    }
} 