#!/bin/bash

# Script to update all repository URLs from supernova-network to mjohnson518

echo "Updating repository URLs in all files..."

# Find all files with the old URL and update them
find . -type f \( -name "*.md" -o -name "*.sh" -o -name "*.yml" -o -name "*.yaml" \) \
  -not -path "./.git/*" \
  -not -path "./target/*" \
  -not -path "./node_modules/*" \
  -exec grep -l "mjohnson518/supernova" {} \; | while read file; do
    echo "Updating: $file"
    sed -i.bak 's|mjohnson518/supernova|mjohnson518/supernova|g' "$file"
    rm "${file}.bak"
done

echo "Update complete!"
echo "Files updated:"
find . -type f \( -name "*.md" -o -name "*.sh" -o -name "*.yml" -o -name "*.yaml" \) \
  -not -path "./.git/*" \
  -not -path "./target/*" \
  -not -path "./node_modules/*" \
  -exec grep -l "mjohnson518/supernova" {} \; | wc -l

echo "Please review the changes before committing." 