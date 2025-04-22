#!/bin/sh
set -e

# Configuration
CHECK_INTERVAL=${CHECK_INTERVAL:-3600}     # Default 1 hour
RETAIN_CHECKPOINTS=${RETAIN_CHECKPOINTS:-14}  # Default 14 days
ARCHIVE_DIR="/archive"
NODES="node1 node2 miner explorer"
DATE_FORMAT="%Y-%m-%d_%H-%M-%S"

# Create archive directory if it doesn't exist
mkdir -p "$ARCHIVE_DIR"

# Function to log messages
log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1"
}

# Function to verify checkpoints for a specific node
verify_node_checkpoints() {
    local node=$1
    local node_dir="/checkpoints/$node"
    local node_archive="$ARCHIVE_DIR/$node"
    local report_file="$node_archive/checkpoint_verification_$(date +"$DATE_FORMAT").log"
    
    mkdir -p "$node_archive"
    
    log "Verifying checkpoints for $node..."
    
    if [ ! -d "$node_dir" ]; then
        log "Checkpoint directory for $node not found, skipping"
        return
    fi
    
    # Initialize report
    {
        echo "Checkpoint Verification Report for $node"
        echo "Generated at: $(date)"
        echo "----------------------------------------"
    } > "$report_file"
    
    # Find all checkpoint directories
    local total_checkpoints=0
    local valid_checkpoints=0
    local invalid_checkpoints=0
    
    for checkpoint_dir in "$node_dir"/checkpoint_*; do
        if [ -d "$checkpoint_dir" ]; then
            total_checkpoints=$((total_checkpoints + 1))
            checkpoint_name=$(basename "$checkpoint_dir")
            info_file="$checkpoint_dir/checkpoint_info.json"
            
            if [ -f "$info_file" ]; then
                # Extract checkpoint info
                height=$(grep -o '"height":[0-9]*' "$info_file" | cut -d':' -f2)
                timestamp=$(grep -o '"timestamp":[0-9]*' "$info_file" | cut -d':' -f2)
                checkpoint_type=$(grep -o '"checkpoint_type":"[^"]*"' "$info_file" | cut -d':' -f2 | tr -d '"')
                
                # Check if data directory exists
                if [ -d "$checkpoint_dir/data" ]; then
                    # For proper verification, we would check the hash integrity here
                    # but for this script, we'll just check if the directory structure looks valid
                    if [ -d "$checkpoint_dir/data" ] && [ "$(ls -A "$checkpoint_dir/data" 2>/dev/null)" ]; then
                        valid_checkpoints=$((valid_checkpoints + 1))
                        status="VALID"
                    else
                        invalid_checkpoints=$((invalid_checkpoints + 1))
                        status="INVALID (Empty data directory)"
                    fi
                else
                    invalid_checkpoints=$((invalid_checkpoints + 1))
                    status="INVALID (Missing data directory)"
                fi
                
                # Add to report
                {
                    echo "Checkpoint: $checkpoint_name"
                    echo "  Height: $height"
                    echo "  Type: $checkpoint_type"
                    echo "  Created: $(date -d @$timestamp 2>/dev/null || date -r $timestamp)"
                    echo "  Status: $status"
                    echo ""
                } >> "$report_file"
            else
                invalid_checkpoints=$((invalid_checkpoints + 1))
                # Add to report
                {
                    echo "Checkpoint: $checkpoint_name"
                    echo "  Status: INVALID (Missing checkpoint_info.json)"
                    echo ""
                } >> "$report_file"
            fi
        fi
    done
    
    # Add summary to report
    {
        echo "----------------------------------------"
        echo "Summary:"
        echo "  Total checkpoints: $total_checkpoints"
        echo "  Valid checkpoints: $valid_checkpoints"
        echo "  Invalid checkpoints: $invalid_checkpoints"
        echo "  Verification time: $(date)"
    } >> "$report_file"
    
    log "Checkpoint verification for $node completed: $valid_checkpoints valid, $invalid_checkpoints invalid"
}

# Function to clean up old checkpoints in the archive
cleanup_old_archives() {
    local node=$1
    local node_archive="$ARCHIVE_DIR/$node"
    local retention_secs=$((RETAIN_CHECKPOINTS * 86400))
    local current_time=$(date +%s)
    
    if [ -d "$node_archive" ]; then
        log "Cleaning up old verification reports for $node..."
        
        find "$node_archive" -name "checkpoint_verification_*.log" -type f | while read report_file; do
            # Get file modification time
            file_time=$(date -r "$report_file" +%s)
            age_secs=$((current_time - file_time))
            
            if [ $age_secs -gt $retention_secs ]; then
                log "Removing old report: $report_file"
                rm -f "$report_file"
            fi
        done
    fi
}

# Main monitoring loop
log "Starting checkpoint monitor. Interval: ${CHECK_INTERVAL}s, Retention: ${RETAIN_CHECKPOINTS} days"

while true; do
    start_time=$(date +%s)
    
    # Verify checkpoints for all nodes
    for node in $NODES; do
        verify_node_checkpoints "$node"
        cleanup_old_archives "$node"
    done
    
    end_time=$(date +%s)
    duration=$((end_time - start_time))
    log "Verification cycle completed in ${duration}s"
    
    # Wait for next cycle
    log "Waiting ${CHECK_INTERVAL}s until next verification cycle"
    sleep $CHECK_INTERVAL
done 