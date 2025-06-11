// Get block details
let block = match node.chain_state().get_block(&block_hash) {
    Some(block) => block,
    None => return HttpResponse::NotFound().json(json!({
        "error": "Block not found"
    }))
};

// Get block height
let height = node.chain_state().get_block_height(&block_hash).unwrap_or(0);

// Calculate confirmations
let confirmations = node.chain_state().get_height().saturating_sub(height) + 1; 