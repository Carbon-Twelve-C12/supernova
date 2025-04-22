#!/bin/sh
set -e

# Configuration
BACKUP_INTERVAL=${BACKUP_INTERVAL:-86400}  # Default 24 hours
RETENTION_DAYS=${RETENTION_DAYS:-7}        # Default 7 days retention
STORAGE_DIR="/storage"
NODES="node1 node2 miner explorer"
DATE_FORMAT="%Y-%m-%d_%H-%M-%S"

# Create storage directory if it doesn't exist
mkdir -p "$STORAGE_DIR"

# Function to log messages
log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1"
}

# Function to create backup for a specific node
backup_node() {
    local node=$1
    local timestamp=$(date +"$DATE_FORMAT")
    local backup_dir="$STORAGE_DIR/$node"
    local backup_file="$backup_dir/${node}_backup_$timestamp.tar.gz"
    
    mkdir -p "$backup_dir"
    
    log "Creating backup for $node..."
    
    if [ -d "/backups/$node" ]; then
        # Create compressed backup
        tar -czf "$backup_file" -C "/backups/$node" .
        
        # Record backup info
        echo "$timestamp" > "$backup_dir/last_backup_time"
        
        log "Backup for $node created successfully: $backup_file"
    else
        log "Backup directory for $node not found, skipping"
    fi
}

# Function to clean up old backups
cleanup_old_backups() {
    local node=$1
    local backup_dir="$STORAGE_DIR/$node"
    local retention_secs=$((RETENTION_DAYS * 86400))
    local current_time=$(date +%s)
    
    if [ -d "$backup_dir" ]; then
        log "Cleaning up old backups for $node..."
        
        find "$backup_dir" -name "${node}_backup_*.tar.gz" -type f | while read backup_file; do
            # Get file modification time
            file_time=$(date -r "$backup_file" +%s)
            age_secs=$((current_time - file_time))
            
            if [ $age_secs -gt $retention_secs ]; then
                log "Removing old backup: $backup_file"
                rm -f "$backup_file"
            fi
        done
    fi
}

# Main backup loop
log "Starting backup manager. Interval: ${BACKUP_INTERVAL}s, Retention: ${RETENTION_DAYS} days"

while true; do
    start_time=$(date +%s)
    
    # Perform backups for all nodes
    for node in $NODES; do
        backup_node "$node"
        cleanup_old_backups "$node"
    done
    
    end_time=$(date +%s)
    duration=$((end_time - start_time))
    log "Backup cycle completed in ${duration}s"
    
    # Wait for next cycle
    log "Waiting ${BACKUP_INTERVAL}s until next backup cycle"
    sleep $BACKUP_INTERVAL
done 