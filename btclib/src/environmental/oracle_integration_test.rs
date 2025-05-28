//! Integration test demonstrating the Environmental Oracle System
//! 
//! This test shows how the oracle system prevents gaming of environmental data
//! by requiring multi-oracle consensus and cryptographic verification.

#[cfg(test)]
mod oracle_integration_tests {
    use std::sync::Arc;
    use std::collections::HashSet;
    use chrono::Utc;
    
    use crate::environmental::{
        emissions::{EmissionsTracker, EmissionsConfig, RECCertificateInfo, VerificationStatus, Region},
        oracle::{EnvironmentalOracle, EnvironmentalData, OracleSubmission, CryptographicProof, EnergySourceType},
    };
    
    #[test]
    fn test_rec_verification_without_oracle_fails() {
        // Create emissions tracker without oracle
        let mut tracker = EmissionsTracker::new(EmissionsConfig::default());
        
        // Create a fake REC certificate
        let fake_rec = RECCertificateInfo {
            certificate_id: "FAKE-REC-123".to_string(),
            issuer: "FakeGreenCerts".to_string(),
            amount_mwh: 1000.0,
            generation_start: Utc::now() - chrono::Duration::days(30),
            generation_end: Utc::now() - chrono::Duration::days(1),
            generation_location: Some(Region::new("US")),
            verification_status: VerificationStatus::None,
            certificate_url: Some("https://fake-certs.com/REC123".to_string()),
        };
        
        // Without oracle, verification should fail or be pending
        let status = tracker.verify_rec_claim(&fake_rec);
        assert_ne!(status, VerificationStatus::Verified);
        assert_eq!(status, VerificationStatus::Pending); // Cannot verify without oracle
    }
    
    #[test]
    fn test_rec_verification_with_oracle_consensus() {
        // Create oracle system with minimum stake of 1000 NOVA
        let oracle_system = Arc::new(EnvironmentalOracle::new(1000));
        
        // Register 3 oracles with different specializations
        oracle_system.register_oracle(
            "oracle1".to_string(),
            2000, // 2000 NOVA stake
            ["rec_certificate".to_string()].into(),
        ).expect("Failed to register oracle1");
        
        oracle_system.register_oracle(
            "oracle2".to_string(),
            2500, // 2500 NOVA stake
            ["rec_certificate", "carbon_offset"].iter().map(|s| s.to_string()).collect(),
        ).expect("Failed to register oracle2");
        
        oracle_system.register_oracle(
            "oracle3".to_string(),
            3000, // 3000 NOVA stake
            ["rec_certificate", "grid_energy_mix"].iter().map(|s| s.to_string()).collect(),
        ).expect("Failed to register oracle3");
        
        // Create emissions tracker with oracle
        let mut tracker = EmissionsTracker::new(EmissionsConfig::default());
        tracker.set_oracle_system(oracle_system.clone());
        
        // Create a REC certificate to verify
        let rec = RECCertificateInfo {
            certificate_id: "REC-2024-SOLAR-001".to_string(),
            issuer: "GreenPowerRegistry".to_string(),
            amount_mwh: 500.0,
            generation_start: Utc::now() - chrono::Duration::days(60),
            generation_end: Utc::now() - chrono::Duration::days(30),
            generation_location: Some(Region::new("US")),
            verification_status: VerificationStatus::None,
            certificate_url: Some("https://greenpower.org/REC-2024-SOLAR-001".to_string()),
        };
        
        // Verify REC through oracle - returns Pending as oracles need to submit data
        let status = tracker.verify_rec_claim(&rec);
        assert_eq!(status, VerificationStatus::Pending);
        
        // In a real system, oracles would:
        // 1. Query external registries to verify the certificate
        // 2. Check blockchain records for double-spending
        // 3. Verify cryptographic signatures
        // 4. Submit their findings with proofs
        
        // Simulate oracle submissions
        let data = EnvironmentalData::RECCertificate {
            certificate_id: rec.certificate_id.clone(),
            issuer: rec.issuer.clone(),
            amount_mwh: rec.amount_mwh,
            generation_start: rec.generation_start,
            generation_end: rec.generation_end,
            location: "US".to_string(),
            energy_type: EnergySourceType::Solar,
            registry_url: rec.certificate_url.clone().unwrap(),
        };
        
        // Create verification request
        let request_id = oracle_system.request_verification(
            data.clone(),
            "miner1".to_string(),
            100, // 100 NOVA bounty
            ["rec_certificate".to_string()].into(),
        ).expect("Failed to create verification request");
        
        // Oracle 1 submits verification
        let submission1 = OracleSubmission {
            oracle_id: "oracle1".to_string(),
            data_type: "rec_certificate".to_string(),
            reference_id: rec.certificate_id.clone(),
            data: data.clone(),
            proof: CryptographicProof {
                proof_type: "signature".to_string(),
                proof_data: vec![1, 2, 3, 4], // Simplified proof
                commitment: [0u8; 32],
                parameters: Default::default(),
            },
            timestamp: Utc::now(),
            signature: vec![5, 6, 7, 8], // Simplified signature
            metadata: Default::default(),
        };
        
        oracle_system.submit_verification(
            "oracle1".to_string(),
            request_id.clone(),
            submission1,
        ).expect("Failed to submit oracle1 verification");
        
        // Oracle 2 submits verification (agreeing)
        let submission2 = OracleSubmission {
            oracle_id: "oracle2".to_string(),
            data_type: "rec_certificate".to_string(),
            reference_id: rec.certificate_id.clone(),
            data: data.clone(),
            proof: CryptographicProof {
                proof_type: "signature".to_string(),
                proof_data: vec![9, 10, 11, 12],
                commitment: [0u8; 32],
                parameters: Default::default(),
            },
            timestamp: Utc::now(),
            signature: vec![13, 14, 15, 16],
            metadata: Default::default(),
        };
        
        oracle_system.submit_verification(
            "oracle2".to_string(),
            request_id.clone(),
            submission2,
        ).expect("Failed to submit oracle2 verification");
        
        // Oracle 3 submits verification (agreeing)
        let submission3 = OracleSubmission {
            oracle_id: "oracle3".to_string(),
            data_type: "rec_certificate".to_string(),
            reference_id: rec.certificate_id.clone(),
            data,
            proof: CryptographicProof {
                proof_type: "signature".to_string(),
                proof_data: vec![17, 18, 19, 20],
                commitment: [0u8; 32],
                parameters: Default::default(),
            },
            timestamp: Utc::now(),
            signature: vec![21, 22, 23, 24],
            metadata: Default::default(),
        };
        
        // This should trigger consensus (3/3 oracles agree)
        oracle_system.submit_verification(
            "oracle3".to_string(),
            request_id.clone(),
            submission3,
        ).expect("Failed to submit oracle3 verification");
        
        // Check verification result
        let result = oracle_system.get_verification_result(&request_id);
        assert!(result.is_some());
        
        let verification = result.unwrap();
        assert_eq!(verification.status, VerificationStatus::Verified);
        assert!(verification.consensus_details.consensus_reached);
        assert_eq!(verification.consensus_details.agreeing_oracles, 3);
        assert_eq!(verification.consensus_details.consensus_percentage, 100.0);
    }
    
    #[test]
    fn test_gaming_attempt_fails_without_consensus() {
        let oracle_system = Arc::new(EnvironmentalOracle::new(1000));
        
        // Register 4 oracles
        for i in 1..=4 {
            oracle_system.register_oracle(
                format!("oracle{}", i),
                2000,
                ["rec_certificate".to_string()].into(),
            ).expect("Failed to register oracle");
        }
        
        // Miner tries to claim fake renewable energy
        let fake_data = EnvironmentalData::RECCertificate {
            certificate_id: "FAKE-REC-999".to_string(),
            issuer: "ScamCerts".to_string(),
            amount_mwh: 10000.0, // Huge amount
            generation_start: Utc::now() - chrono::Duration::days(10),
            generation_end: Utc::now(),
            location: "XX".to_string(), // Fake location
            energy_type: EnergySourceType::Solar,
            registry_url: "https://scam.com".to_string(),
        };
        
        let request_id = oracle_system.request_verification(
            fake_data.clone(),
            "scammer1".to_string(),
            100,
            ["rec_certificate".to_string()].into(),
        ).expect("Failed to create request");
        
        // Only 1 corrupt oracle agrees (25%)
        let corrupt_submission = OracleSubmission {
            oracle_id: "oracle1".to_string(),
            data_type: "rec_certificate".to_string(),
            reference_id: "FAKE-REC-999".to_string(),
            data: fake_data.clone(),
            proof: CryptographicProof {
                proof_type: "fake".to_string(),
                proof_data: vec![99, 99, 99],
                commitment: [99u8; 32],
                parameters: Default::default(),
            },
            timestamp: Utc::now(),
            signature: vec![99, 99, 99, 99],
            metadata: Default::default(),
        };
        
        oracle_system.submit_verification(
            "oracle1".to_string(),
            request_id.clone(),
            corrupt_submission,
        ).expect("Failed to submit");
        
        // Other 3 oracles reject (report different data showing it's fake)
        let reject_data = EnvironmentalData::RECCertificate {
            certificate_id: "FAKE-REC-999".to_string(),
            issuer: "N/A - Certificate Not Found".to_string(),
            amount_mwh: 0.0,
            generation_start: Utc::now(),
            generation_end: Utc::now(),
            location: "".to_string(),
            energy_type: EnergySourceType::Other,
            registry_url: "".to_string(),
        };
        
        for i in 2..=4 {
            let honest_submission = OracleSubmission {
                oracle_id: format!("oracle{}", i),
                data_type: "rec_certificate".to_string(),
                reference_id: "FAKE-REC-999".to_string(),
                data: reject_data.clone(),
                proof: CryptographicProof {
                    proof_type: "registry_check".to_string(),
                    proof_data: vec![0, 0, 0, 0],
                    commitment: [0u8; 32],
                    parameters: Default::default(),
                },
                timestamp: Utc::now(),
                signature: vec![i as u8, i as u8, i as u8, i as u8],
                metadata: Default::default(),
            };
            
            let res = oracle_system.submit_verification(
                format!("oracle{}", i),
                request_id.clone(),
                honest_submission,
            );
            
            if i == 3 {
                // Should trigger consensus check on 3rd submission
                assert!(res.is_ok());
            } else if i == 4 {
                // 4th submission should succeed as consensus already processed
                assert!(res.is_ok());
            }
        }
        
        // Check that consensus was reached for rejection (75% agree it's fake)
        let result = oracle_system.get_verification_result(&request_id);
        assert!(result.is_some());
        
        let verification = result.unwrap();
        assert_eq!(verification.status, VerificationStatus::Verified);
        assert!(verification.consensus_details.consensus_reached);
        assert_eq!(verification.consensus_details.agreeing_oracles, 3);
        assert_eq!(verification.consensus_details.disagreeing_oracles, 1);
        
        // Oracle 1 would be penalized for false verification
        let (oracle1_info, oracle1_metrics) = oracle_system.get_oracle_stats("oracle1").unwrap();
        assert_eq!(oracle1_info.incorrect_verifications, 1);
        assert!(oracle1_info.reputation_score < 500); // Started at 500, now lower
    }
    
    #[test]
    fn test_oracle_slashing_for_misbehavior() {
        let oracle_system = Arc::new(EnvironmentalOracle::new(1000));
        
        // Register oracle with minimum stake
        oracle_system.register_oracle(
            "bad_oracle".to_string(),
            1000, // Minimum stake
            ["rec_certificate".to_string()].into(),
        ).expect("Failed to register");
        
        // Oracle repeatedly provides false data (simulated by slashing)
        oracle_system.slash_oracle(
            "bad_oracle",
            500,
            "Provided false REC verification".to_string(),
        ).expect("Failed to slash");
        
        // Check oracle status
        let (info, _) = oracle_system.get_oracle_stats("bad_oracle").unwrap();
        assert_eq!(info.stake_amount, 500); // Half stake remaining
        assert!(info.reputation_score < 500); // Reputation damaged
        assert!(info.is_active); // Still active with 500 NOVA
        
        // Slash again - should deactivate
        oracle_system.slash_oracle(
            "bad_oracle",
            500,
            "Repeated false verification".to_string(),
        ).expect("Failed to slash");
        
        let (info, _) = oracle_system.get_oracle_stats("bad_oracle").unwrap();
        assert_eq!(info.stake_amount, 0); // No stake left
        assert!(!info.is_active); // Deactivated
        assert_eq!(info.slashing_events.len(), 2); // Two slashing events
    }
} 