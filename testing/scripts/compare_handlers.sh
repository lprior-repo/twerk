#!/bin/bash
# compare_handlers.sh - Compare Go handlers vs Rust handlers

GO_DIR="/tmp/tork"
RUST_FILE="/home/lewis/src/twerk/crates/twerk-app/src/engine/coordinator/handlers.rs"

echo "=============================================="
echo "  HANDLER FUNCTIONS COMPARISON"
echo "=============================================="

echo ""
echo "=== GO HANDLER FUNCTIONS ==="
echo "Files in /tmp/tork/internal/coordinator/handlers/:"
ls -la "$GO_DIR/internal/coordinator/handlers/"

echo ""
echo "Function counts by file:"
for f in "$GO_DIR/internal/coordinator/handlers/"*.go; do
    name=$(basename "$f")
    count=$(grep -c "^func " "$f" 2>/dev/null || echo "0")
    lines=$(wc -l < "$f")
    echo "  $name: $count funcs, $lines lines"
done

echo ""
echo "ALL GO HANDLER FUNCTIONS:"
grep -h "^func " "$GO_DIR/internal/coordinator/handlers/"*.go | sed 's/func /- /' | sed 's/(.*$//'

echo ""
echo "=== RUST HANDLER FUNCTIONS ==="
echo "File: $RUST_FILE"
if [ -f "$RUST_FILE" ]; then
    lines=$(wc -l < "$RUST_FILE")
    echo "Total lines: $lines"
    echo ""
    echo "Functions:"
    grep "^pub async fn \|^async fn \|^fn " "$RUST_FILE" | sed 's/$/;/' | head -50
else
    echo "FILE NOT FOUND"
fi

echo ""
echo "=== KEY MISSING FUNCTIONS ==="
echo "Checking if these Go functions exist in Rust:"
for func in "NewCancelHandler" "NewJobSchedulerHandler" "NewRedeliveredHandler" "NewStartedHandler" "NewProgressHandler" "NewHeartbeatHandler" "NewLogHandler" "NewPendingHandler"; do
    echo -n "  $func: "
    grep -q "$func" "$RUST_FILE" 2>/dev/null && echo "FOUND" || echo "MISSING"
done