#!/bin/bash
# Stop all AFL++ fuzzing instances

echo "Stopping all AFL++ instances..."

# Kill all afl-fuzz processes
pkill -f afl-fuzz

# Kill all screen sessions
screen -ls | grep afl | cut -d. -f1 | awk '{print $1}' | xargs -I{} screen -X -S {} quit

echo "All fuzzing instances stopped."

# Show any crashes found
echo ""
echo "Checking for crashes..."
for dir in findings/*/fuzzer*/crashes; do
    if [ -d "$dir" ] && [ "$(ls -A $dir 2>/dev/null)" ]; then
        echo "Crashes found in: $dir"
        ls -la "$dir" | grep -v "README.txt"
    fi
done

echo ""
echo "To analyze crashes:"
echo "  ./analyze_crashes.sh <target>"