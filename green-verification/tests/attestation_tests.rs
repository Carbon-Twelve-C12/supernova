//! End-to-end attestation verification tests using real ML-DSA (Dilithium3)
//! keys from supernova-core. UNAUDITED / TESTNET-ONLY prototype.

use green_verification::attestation::{GreenAttestation, OracleSig, VerifyParams};
use green_verification::error::GreenError;
use green_verification::oracle::Committee;
use green_verification::types::{EnergyType, MinerAddr, OracleId, RegistryId};
use supernova_core::crypto::{MLDSAPrivateKey, MLDSAPublicKey, MLDSASecurityLevel};

const N: usize = 21;
const M: usize = 15;

/// Build N Level3 keypairs and a committee over their public keys.
fn build_committee() -> (Vec<(OracleId, MLDSAPrivateKey)>, Committee) {
    let mut rng = rand::rngs::OsRng;
    let mut keys: Vec<(OracleId, MLDSAPrivateKey)> = Vec::with_capacity(N);
    let mut members: Vec<(OracleId, MLDSAPublicKey)> = Vec::with_capacity(N);
    for i in 0..N {
        let sk = MLDSAPrivateKey::generate(&mut rng, MLDSASecurityLevel::Level3)
            .expect("keygen should succeed");
        let mut id_bytes = [0u8; 32];
        id_bytes[0] = i as u8;
        let id = OracleId(id_bytes);
        members.push((id, sk.public_key()));
        keys.push((id, sk));
    }
    (keys, Committee::new(1, members, M))
}

/// A fresh, in-window, in-bound base attestation with no signatures yet.
fn base_attestation() -> GreenAttestation {
    GreenAttestation {
        version: 1,
        eac_retirement_id: [42u8; 32],
        registry_id: RegistryId(1),
        mwh_milli: 5_000,
        vintage_epoch: 100,
        energy_type: EnergyType::Solar,
        miner_pqc_addr: MinerAddr([9u8; 32]),
        nonce: [1u8; 32],
        expiry_height: 1_000,
        time_match: None,
        meter_sig: None,
        committee_epoch: 1,
        signatures: Vec::new(),
    }
}

fn params() -> VerifyParams {
    VerifyParams {
        vintage_lo: 0,
        vintage_hi: 1_000,
        max_claimable_mwh_milli: 10_000,
    }
}

/// Sign `att`'s digest with the first `count` distinct committee keys.
fn sign_with(att: &GreenAttestation, keys: &[(OracleId, MLDSAPrivateKey)], count: usize) -> Vec<OracleSig> {
    let digest = att.canonical_digest();
    keys.iter()
        .take(count)
        .map(|(id, sk)| OracleSig {
            oracle_id: *id,
            sig: sk.sign(&digest).expect("sign should succeed"),
        })
        .collect()
}

#[test]
fn quorum_of_m_verifies() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.signatures = sign_with(&att, &keys, M);
    assert_eq!(att.verify(&committee, 500, &params()), Ok(()));
}

#[test]
fn below_threshold_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.signatures = sign_with(&att, &keys, M - 1);
    assert_eq!(
        att.verify(&committee, 500, &params()),
        Err(GreenError::QuorumNotMet { have: M - 1, need: M })
    );
}

#[test]
fn duplicate_signer_counts_once() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    // M-1 distinct signers, plus one duplicate of signer 0 => only M-1 distinct.
    let mut sigs = sign_with(&att, &keys, M - 1);
    let digest = att.canonical_digest();
    sigs.push(OracleSig {
        oracle_id: keys[0].0,
        sig: keys[0].1.sign(&digest).unwrap(),
    });
    att.signatures = sigs;
    assert_eq!(
        att.verify(&committee, 500, &params()),
        Err(GreenError::QuorumNotMet { have: M - 1, need: M })
    );
}

#[test]
fn unknown_oracle_key_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    let mut sigs = sign_with(&att, &keys, M);
    // Replace one signer's id with an id not in the committee.
    sigs[0].oracle_id = OracleId([0xFF; 32]);
    att.signatures = sigs;
    assert_eq!(
        att.verify(&committee, 500, &params()),
        Err(GreenError::UnknownOracleKey)
    );
}

#[test]
fn wrong_key_signature_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    let mut sigs = sign_with(&att, &keys, M);
    // Signer[0] claims id of member 0 but signs a garbage message -> invalid.
    let bad = keys[0].1.sign(b"not the digest").unwrap();
    sigs[0].sig = bad;
    att.signatures = sigs;
    assert_eq!(
        att.verify(&committee, 500, &params()),
        Err(GreenError::InvalidSignature)
    );
}

#[test]
fn tampered_field_invalidates_signatures() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.signatures = sign_with(&att, &keys, M);
    // Tamper with a signed field after signing: the digest changes, so every
    // signature is now over the wrong message.
    att.mwh_milli += 1;
    assert_eq!(
        att.verify(&committee, 500, &params()),
        Err(GreenError::InvalidSignature)
    );
}

#[test]
fn expired_attestation_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.expiry_height = 100;
    att.signatures = sign_with(&att, &keys, M);
    assert_eq!(
        att.verify(&committee, 100, &params()),
        Err(GreenError::ExpiredAttestation { expiry: 100, height: 100 })
    );
}

#[test]
fn vintage_out_of_window_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.vintage_epoch = 5_000;
    att.signatures = sign_with(&att, &keys, M);
    let err = att.verify(&committee, 500, &params()).unwrap_err();
    assert!(matches!(err, GreenError::VintageOutOfWindow { .. }));
}

#[test]
fn mwh_over_bound_is_rejected() {
    let (keys, committee) = build_committee();
    let mut att = base_attestation();
    att.mwh_milli = 999_999;
    att.signatures = sign_with(&att, &keys, M);
    let err = att.verify(&committee, 500, &params()).unwrap_err();
    assert!(matches!(err, GreenError::MwhExceedsBound { .. }));
}
