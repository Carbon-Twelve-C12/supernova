#!/bin/bash
# Automated unwrap() removal script for Supernova
# This script fixes common unwrap patterns with proper error handling

echo "🔧 Starting automated unwrap() removal..."

# Function to fix lock().unwrap() patterns
fix_lock_unwraps() {
    echo "Fixing lock().unwrap() patterns..."
    find . -name "*.rs" -type f ! -path "./target/*" ! -path "./tests/*" -exec sed -i '' \
        's/\.lock()\.unwrap()/\.lock().map_err(|e| StorageError::LockError(e.to_string()))?/g' {} \;
}

# Function to fix parse().unwrap() patterns
fix_parse_unwraps() {
    echo "Fixing parse().unwrap() patterns..."
    find . -name "*.rs" -type f ! -path "./target/*" ! -path "./tests/*" -exec sed -i '' \
        's/\.parse()\.unwrap()/\.parse().map_err(|e| ValidationError::ParseError(e.to_string()))?/g' {} \;
}

# Function to fix simple unwrap() to ? conversion
fix_simple_unwraps() {
    echo "Fixing simple unwrap() to ? patterns..."
    find . -name "*.rs" -type f ! -path "./target/*" ! -path "./tests/*" -exec sed -i '' \
        's/\.unwrap()/?/g' {} \;
}

# Function to add necessary imports
add_error_imports() {
    echo "Adding error imports to files..."
    for file in $(find . -name "*.rs" -type f ! -path "./target/*" ! -path "./tests/*"); do
        # Check if file uses our error types
        if grep -q "StorageError\|ValidationError" "$file"; then
            # Check if imports are missing
            if ! grep -q "use.*error::{" "$file"; then
                # Add import at the beginning of the file after other use statements
                sed -i '' '1,/^use/ { /^use/a\
use crate::error::{StorageError, ValidationError};
}' "$file"
            fi
        fi
    done
}

# Main execution
echo "Phase 1: Fixing lock().unwrap() patterns..."
fix_lock_unwraps

echo "Phase 2: Fixing parse().unwrap() patterns..."
fix_parse_unwraps

echo "Phase 3: Adding error imports..."
add_error_imports

echo "✅ Automated fixes complete!"
echo "Note: Manual review is still required for:"
echo "  - Complex unwrap patterns"
echo "  - expect() calls"
echo "  - panic! statements"
echo "  - Context-specific error handling" 