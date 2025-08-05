//! Fuzzing harness for P2P network message parsing
//! 
//! This harness tests the P2P protocol message parsing to ensure it handles
//! malformed, oversized, and malicious messages without panicking.

use afl::fuzz;
use btclib::network::messages::{
    NetworkMessage, MessageHeader, MessageType,
    BlockMessage, TransactionMessage, GetBlocksMessage,
    GetDataMessage, InvMessage, AddrMessage, PingMessage
};
use btclib::network::protocol::deserialize_message;

fn main() {
    fuzz!(|data: &[u8]| {
        // Test raw message deserialization
        fuzz_message_deserialization(data);
        
        // Test specific message types
        if !data.is_empty() {
            match data[0] % 8 {
                0 => fuzz_block_message(data),
                1 => fuzz_transaction_message(data),
                2 => fuzz_inv_message(data),
                3 => fuzz_addr_message(data),
                4 => fuzz_getblocks_message(data),
                5 => fuzz_getdata_message(data),
                6 => fuzz_ping_pong_messages(data),
                7 => fuzz_header_parsing(data),
                _ => unreachable!(),
            }
        }
    });
}

/// Test raw message deserialization
fn fuzz_message_deserialization(data: &[u8]) {
    // Test with various data sizes
    for chunk_size in [24, 100, 1000, 10000, 100000] {
        if data.len() >= chunk_size {
            let chunk = &data[..chunk_size];
            
            // This should never panic, only return errors
            match deserialize_message(chunk) {
                Ok(msg) => {
                    // Test message validation
                    validate_message(&msg);
                    
                    // Test serialization round-trip
                    test_serialization_roundtrip(&msg);
                }
                Err(_) => {
                    // Deserialization errors are expected for fuzzing
                }
            }
        }
    }
}

/// Fuzz block message parsing
fn fuzz_block_message(data: &[u8]) {
    if data.len() < 80 {  // Minimum block header size
        return;
    }
    
    // Create a block message from fuzzer data
    match BlockMessage::from_bytes(data) {
        Ok(msg) => {
            // Validate block message constraints
            test_block_message_validation(&msg);
            
            // Test block size limits
            if msg.block_size() > 4_000_000 {  // 4MB block size limit
                // Should be rejected in validation
                assert!(validate_block_size(&msg).is_err());
            }
        }
        Err(_) => {}
    }
}

/// Fuzz transaction message parsing
fn fuzz_transaction_message(data: &[u8]) {
    match TransactionMessage::from_bytes(data) {
        Ok(msg) => {
            // Test transaction validation
            test_transaction_message_validation(&msg);
            
            // Test script size limits
            test_script_size_limits(&msg);
            
            // Test witness data parsing
            test_witness_parsing(&msg);
        }
        Err(_) => {}
    }
}

/// Fuzz inventory message parsing
fn fuzz_inv_message(data: &[u8]) {
    match InvMessage::from_bytes(data) {
        Ok(msg) => {
            // Test inventory limits (50,000 items max)
            if msg.inventory.len() > 50_000 {
                // Should be rejected
                assert!(validate_inv_message(&msg).is_err());
            }
            
            // Test each inventory item
            for inv_item in &msg.inventory {
                test_inventory_item(inv_item);
            }
        }
        Err(_) => {}
    }
}

/// Fuzz address message parsing
fn fuzz_addr_message(data: &[u8]) {
    match AddrMessage::from_bytes(data) {
        Ok(msg) => {
            // Test address count limits (1,000 addresses max)
            if msg.addresses.len() > 1_000 {
                assert!(validate_addr_message(&msg).is_err());
            }
            
            // Test each address
            for addr in &msg.addresses {
                test_network_address(addr);
            }
        }
        Err(_) => {}
    }
}

/// Fuzz getblocks message parsing
fn fuzz_getblocks_message(data: &[u8]) {
    match GetBlocksMessage::from_bytes(data) {
        Ok(msg) => {
            // Test protocol version
            test_protocol_version(msg.version);
            
            // Test block locator limits
            if msg.block_locators.len() > 500 {
                assert!(validate_getblocks_message(&msg).is_err());
            }
            
            // Test hash format
            for hash in &msg.block_locators {
                test_hash_format(hash);
            }
        }
        Err(_) => {}
    }
}

/// Fuzz getdata message parsing
fn fuzz_getdata_message(data: &[u8]) {
    match GetDataMessage::from_bytes(data) {
        Ok(msg) => {
            // Similar to inv message, test limits
            if msg.inventory.len() > 50_000 {
                assert!(validate_getdata_message(&msg).is_err());
            }
        }
        Err(_) => {}
    }
}

/// Fuzz ping/pong message parsing
fn fuzz_ping_pong_messages(data: &[u8]) {
    // Test ping message
    if data.len() >= 8 {
        let nonce = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7]
        ]);
        
        let ping = PingMessage::new(nonce);
        test_ping_message(&ping);
        
        // Test corresponding pong
        let pong = ping.create_pong();
        test_pong_message(&pong);
    }
}

/// Fuzz message header parsing
fn fuzz_header_parsing(data: &[u8]) {
    if data.len() >= 24 {  // Message header size
        match MessageHeader::from_bytes(&data[..24]) {
            Ok(header) => {
                // Test magic bytes validation
                test_magic_bytes(&header);
                
                // Test command string validation
                test_command_validation(&header);
                
                // Test payload size limits
                if header.payload_size > 32_000_000 {  // 32MB max
                    assert!(validate_header(&header).is_err());
                }
                
                // Test checksum validation
                if data.len() >= 24 + header.payload_size as usize {
                    let payload = &data[24..24 + header.payload_size as usize];
                    test_checksum_validation(&header, payload);
                }
            }
            Err(_) => {}
        }
    }
}

/// Validate a parsed message
fn validate_message(msg: &NetworkMessage) -> Result<(), &'static str> {
    // Implement comprehensive message validation
    match msg {
        NetworkMessage::Block(block_msg) => validate_block_message(block_msg),
        NetworkMessage::Transaction(tx_msg) => validate_transaction_message(tx_msg),
        NetworkMessage::Inv(inv_msg) => validate_inv_message(inv_msg),
        NetworkMessage::Addr(addr_msg) => validate_addr_message(addr_msg),
        _ => Ok(()),
    }
}

/// Test serialization round-trip
fn test_serialization_roundtrip(msg: &NetworkMessage) {
    match msg.to_bytes() {
        Ok(bytes) => {
            // Should be able to parse back
            match NetworkMessage::from_bytes(&bytes) {
                Ok(parsed) => {
                    // Messages should be equal
                    // Note: Implement PartialEq for NetworkMessage
                }
                Err(_) => {
                    // Round-trip failure indicates a bug
                    panic!("Serialization round-trip failed");
                }
            }
        }
        Err(_) => {
            // Serialization failure is acceptable for some edge cases
        }
    }
}

// Validation helper functions (stubs for the example)
fn validate_block_size(msg: &BlockMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_block_message(msg: &BlockMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_transaction_message(msg: &TransactionMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_inv_message(msg: &InvMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_addr_message(msg: &AddrMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_getblocks_message(msg: &GetBlocksMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_getdata_message(msg: &GetDataMessage) -> Result<(), &'static str> { Ok(()) }
fn validate_header(header: &MessageHeader) -> Result<(), &'static str> { Ok(()) }

fn test_block_message_validation(msg: &BlockMessage) {}
fn test_transaction_message_validation(msg: &TransactionMessage) {}
fn test_script_size_limits(msg: &TransactionMessage) {}
fn test_witness_parsing(msg: &TransactionMessage) {}
fn test_inventory_item(item: &btclib::network::messages::InventoryItem) {}
fn test_network_address(addr: &btclib::network::messages::NetworkAddress) {}
fn test_protocol_version(version: u32) {}
fn test_hash_format(hash: &[u8; 32]) {}
fn test_ping_message(ping: &PingMessage) {}
fn test_pong_message(pong: &btclib::network::messages::PongMessage) {}
fn test_magic_bytes(header: &MessageHeader) {}
fn test_command_validation(header: &MessageHeader) {}
fn test_checksum_validation(header: &MessageHeader, payload: &[u8]) {}