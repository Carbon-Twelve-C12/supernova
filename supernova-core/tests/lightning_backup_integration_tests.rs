//! Lightning channel backup — P2P integration tests (Phase 1 A2).
//!
//! Proves that two `PeerBackupProtocol` instances wired together through an
//! in-process transport can:
//!   * replicate an encrypted backup from Alice to Bob,
//!   * recover that backup on a fresh Alice instance after simulated data loss,
//!   * handle request/response correlation, timeouts, and missing blobs correctly.
//!
//! The in-process transport stands in for the real Lightning wire; production
//! integrations implement [`PeerBackupTransport`] on top of the Lightning
//! message channel (see `supernova-core/src/lightning/wire.rs`).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use supernova_core::lightning::backup::{
    BackupError, BackupResult, ChannelBackupPackage, ChannelType, EncryptedBackup,
    FundingOutpoint, PeerBackupMessage, PeerBackupProtocol, PeerBackupTransport,
    StaticChannelBackup,
};
use supernova_core::lightning::channel::ChannelId;
use supernova_core::lightning::router::NodeId;

/// In-process transport that routes `PeerBackupMessage`s directly between a
/// map of `(NodeId -> PeerBackupProtocol)` instances.
#[derive(Clone)]
struct InMemoryTransport {
    /// The sender's node id (attached to each exchange so the receiver knows who we are).
    from: NodeId,
    /// Shared registry of peers reachable on this bus.
    peers: Arc<Mutex<HashMap<NodeId, Arc<PeerBackupProtocol>>>>,
    /// Optional failure injection: peers that always error on exchange.
    unreachable: Arc<Mutex<Vec<NodeId>>>,
    /// Optional slow peers: (peer, seconds-to-sleep) — simulate a hang longer
    /// than the caller's timeout without blocking the whole test too long.
    stalled: Arc<Mutex<Vec<(NodeId, u64)>>>,
}

impl InMemoryTransport {
    fn new(from: NodeId) -> Self {
        Self {
            from,
            peers: Arc::new(Mutex::new(HashMap::new())),
            unreachable: Arc::new(Mutex::new(Vec::new())),
            stalled: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn register(&self, node: Arc<PeerBackupProtocol>) {
        self.peers
            .lock()
            .unwrap()
            .insert(node.our_node_id().clone(), node);
    }

    fn mark_unreachable(&self, peer: NodeId) {
        self.unreachable.lock().unwrap().push(peer);
    }

    fn mark_stalled(&self, peer: NodeId, secs: u64) {
        self.stalled.lock().unwrap().push((peer, secs));
    }
}

#[async_trait]
impl PeerBackupTransport for InMemoryTransport {
    async fn exchange(
        &self,
        peer: &NodeId,
        message: PeerBackupMessage,
    ) -> BackupResult<PeerBackupMessage> {
        if self.unreachable.lock().unwrap().contains(peer) {
            return Err(BackupError::PeerBackupFailed {
                peer_id: peer.to_string(),
                reason: "simulated unreachable peer".to_string(),
            });
        }

        let stall_secs = self
            .stalled
            .lock()
            .unwrap()
            .iter()
            .find_map(|(p, s)| if p == peer { Some(*s) } else { None });
        if let Some(secs) = stall_secs {
            // Park longer than the protocol timeout so the caller hits its deadline.
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }

        let target = {
            let guard = self.peers.lock().unwrap();
            guard.get(peer).cloned()
        };

        let Some(target) = target else {
            return Err(BackupError::PeerBackupFailed {
                peer_id: peer.to_string(),
                reason: "peer not on bus".to_string(),
            });
        };

        match target.handle_message(&self.from, message)? {
            Some(reply) => Ok(reply),
            None => Err(BackupError::PeerBackupFailed {
                peer_id: peer.to_string(),
                reason: "peer produced no reply".to_string(),
            }),
        }
    }
}

fn sample_package(owner: &NodeId) -> ChannelBackupPackage {
    let scb = StaticChannelBackup {
        channel_id: ChannelId::from_bytes([7u8; 32]),
        remote_node_id: NodeId::new("remote".to_string()),
        capacity_sats: 250_000,
        funding_outpoint: FundingOutpoint {
            txid: [3u8; 32],
            vout: 1,
        },
        derivation_path: vec![44, 1, 0, 7],
        channel_type: ChannelType::default(),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        version: 1,
    };
    ChannelBackupPackage::new(owner.clone(), vec![scb])
}

fn test_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(11);
    }
    key
}

#[tokio::test]
async fn peer_backup_round_trip_replication_and_recovery() {
    let alice_id = NodeId::new("alice".to_string());
    let bob_id = NodeId::new("bob".to_string());

    // Bob hosts backups for Alice; Alice trusts Bob.
    let bob = Arc::new(PeerBackupProtocol::new(bob_id.clone(), Vec::new()));

    let alice_transport = Arc::new(InMemoryTransport::new(alice_id.clone()));
    alice_transport.register(bob.clone());

    let alice = PeerBackupProtocol::new(alice_id.clone(), vec![bob_id.clone()])
        .with_transport(alice_transport.clone() as Arc<dyn PeerBackupTransport>);

    // Alice encrypts her backup and ships it to Bob.
    let key = test_key();
    let package = sample_package(&alice_id);
    let encrypted = EncryptedBackup::encrypt(&package, &key).expect("encrypt");

    let accepted = alice
        .distribute_backup_async(&encrypted, "alice-rc4")
        .await
        .expect("distribute must succeed");
    assert_eq!(accepted, 1, "Bob must acknowledge the backup");

    // Simulate total data loss: brand-new Alice protocol, only Bob knows the ciphertext.
    let alice_recovered_transport = Arc::new(InMemoryTransport::new(alice_id.clone()));
    alice_recovered_transport.register(bob.clone());
    let alice_recovered = PeerBackupProtocol::new(alice_id.clone(), vec![bob_id.clone()])
        .with_transport(alice_recovered_transport as Arc<dyn PeerBackupTransport>);

    let pulled = alice_recovered
        .request_backup_from_peers_async("alice-rc4")
        .await
        .expect("recovery must succeed");

    let decoded = pulled.decrypt(&key).expect("decrypt recovered backup");
    assert!(decoded.verify(), "checksum on recovered package");
    assert_eq!(decoded.channels.len(), 1);
    assert_eq!(decoded.channels[0].capacity_sats, 250_000);
}

#[tokio::test]
async fn peer_backup_missing_blob_reports_not_found() {
    let alice_id = NodeId::new("alice".to_string());
    let bob_id = NodeId::new("bob".to_string());

    let bob = Arc::new(PeerBackupProtocol::new(bob_id.clone(), Vec::new()));
    let transport = Arc::new(InMemoryTransport::new(alice_id.clone()));
    transport.register(bob);

    let alice = PeerBackupProtocol::new(alice_id, vec![bob_id])
        .with_transport(transport as Arc<dyn PeerBackupTransport>);

    let err = alice
        .request_backup_from_peers_async("never-stored")
        .await
        .expect_err("missing backup must be reported as error");

    match err {
        BackupError::BackupNotFound { backup_id } => assert_eq!(backup_id, "never-stored"),
        other => panic!("unexpected error variant: {:?}", other),
    }
}

#[tokio::test]
async fn peer_backup_unreachable_peer_is_skipped() {
    let alice_id = NodeId::new("alice".to_string());
    let bob_id = NodeId::new("bob".to_string());
    let carol_id = NodeId::new("carol".to_string());

    // Carol has the backup; Bob is unreachable.
    let bob = Arc::new(PeerBackupProtocol::new(bob_id.clone(), Vec::new()));
    let carol = Arc::new(PeerBackupProtocol::new(carol_id.clone(), Vec::new()));

    let transport = Arc::new(InMemoryTransport::new(alice_id.clone()));
    transport.register(bob.clone());
    transport.register(carol.clone());
    transport.mark_unreachable(bob_id.clone());

    // Seed Carol with the backup directly — she's hosting it for Alice.
    let key = test_key();
    let package = sample_package(&alice_id);
    let encrypted = EncryptedBackup::encrypt(&package, &key).unwrap();
    carol
        .receive_peer_backup(encrypted.clone(), "alice-rc4".to_string())
        .unwrap();

    let alice = PeerBackupProtocol::new(alice_id, vec![bob_id, carol_id])
        .with_transport(transport as Arc<dyn PeerBackupTransport>);

    // Alice should skip unreachable Bob and succeed via Carol.
    let pulled = alice
        .request_backup_from_peers_async("alice-rc4")
        .await
        .expect("Carol must answer after Bob fails");
    let decoded = pulled.decrypt(&key).unwrap();
    assert!(decoded.verify());
}

#[tokio::test]
async fn peer_backup_stalled_peer_hits_timeout() {
    // Protocol timeout = 1s; peer stalls for 3s; distribution must fail by timeout.
    let alice_id = NodeId::new("alice".to_string());
    let bob_id = NodeId::new("bob".to_string());

    let bob = Arc::new(PeerBackupProtocol::new(bob_id.clone(), Vec::new()));
    let transport = Arc::new(InMemoryTransport::new(alice_id.clone()));
    transport.register(bob);
    transport.mark_stalled(bob_id.clone(), 3);

    let key = test_key();
    let package = sample_package(&alice_id);
    let encrypted = EncryptedBackup::encrypt(&package, &key).unwrap();

    let alice = PeerBackupProtocol::new(alice_id, vec![bob_id])
        .with_transport(transport as Arc<dyn PeerBackupTransport>)
        .with_timeout_secs(1);

    let result = alice
        .distribute_backup_async(&encrypted, "alice-stall")
        .await;

    assert!(
        matches!(result, Err(BackupError::PeerBackupFailed { .. })),
        "expected PeerBackupFailed on timeout, got {:?}",
        result,
    );
}

#[tokio::test]
async fn transport_not_configured_rejects_async_send() {
    // No transport means no wire — async sends must refuse, not silently succeed.
    let alice_id = NodeId::new("alice".to_string());
    let bob_id = NodeId::new("bob".to_string());

    let alice = PeerBackupProtocol::new(alice_id.clone(), vec![bob_id.clone()]);
    let key = test_key();
    let package = sample_package(&alice_id);
    let encrypted = EncryptedBackup::encrypt(&package, &key).unwrap();

    let err = alice
        .send_to_peer_async(&bob_id, &encrypted, "alice-no-wire")
        .await
        .expect_err("send without transport must fail");

    match err {
        BackupError::PeerBackupFailed { reason, .. } => {
            assert!(
                reason.contains("transport not configured"),
                "unexpected reason: {}",
                reason
            );
        }
        other => panic!("unexpected variant: {:?}", other),
    }
}

#[tokio::test]
async fn handle_message_rejects_spurious_response() {
    // A Response arriving through the dispatcher (rather than the correlated client)
    // must be dropped, not treated as a request.
    let bob_id = NodeId::new("bob".to_string());
    let bob = PeerBackupProtocol::new(bob_id, Vec::new());

    let reply = bob
        .handle_message(
            &NodeId::new("stranger".to_string()),
            PeerBackupMessage::Response {
                request_id: 42,
                backup_id: "unsolicited".to_string(),
                backup: None,
            },
        )
        .expect("handler must not error");
    assert!(reply.is_none(), "spurious Response must yield no reply");
}
