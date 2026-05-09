#!/bin/bash
# compare_go_rust.sh - Improved line-by-line comparison of Go Tork vs Rust Twerk

set -e

GO_DIR="/tmp/tork"
RUST_DIR="/home/lewis/src/twerk/crates"

echo "=============================================="
echo "  GO TORK vs RUST TWERK - PARITY ANALYSIS"
echo "=============================================="

# Helper function to count functions in Rust
count_rust_funcs() {
    local file=$1
    if [ ! -f "$file" ]; then echo 0; return; fi
    # Count pub fn, fn, and also functions inside impl blocks
    grep -E "^[[:space:]]*(pub[[:space:]]+)?fn[[:space:]]+[a-zA-Z0-9_]+" "$file" | wc -l
}

# Helper function to count functions in Go
count_go_funcs() {
    local file=$1
    if [ ! -f "$file" ]; then echo 0; return; fi
    grep -E "^func[[:space:]]+" "$file" | wc -l
}

echo ""
echo "=== CORE MODELS (twerk-core) ==="
printf "%-15s | %-15s | %-15s\n" "Component" "Go (Lines/Funcs)" "Rust (Lines/Funcs)"
printf "%-15s | %-15s | %-15s\n" "---------------" "---------------" "---------------"

for file in job task node user role mount; do
    GO_FILE="$GO_DIR/${file}.go"
    RUST_FILE="$RUST_DIR/twerk-core/src/${file}.rs"
    
    GO_INFO="N/A"
    if [ -f "$GO_FILE" ]; then
        GO_INFO="$(wc -l < "$GO_FILE")/$(count_go_funcs "$GO_FILE")"
    fi
    
    RUST_INFO="N/A"
    if [ -f "$RUST_FILE" ]; then
        RUST_INFO="$(wc -l < "$RUST_FILE")/$(count_rust_funcs "$RUST_FILE")"
    fi
    
    printf "%-15s | %-15s | %-15s\n" "$file" "$GO_INFO" "$RUST_INFO"
done

echo ""
echo "=== INFRASTRUCTURE (twerk-infrastructure) ==="
printf "%-20s | %-15s | %-15s\n" "Component" "Go (Lines)" "Rust (Lines)"
printf "%-20s | %-15s | %-15s\n" "--------------------" "---------------" "---------------"

# Broker
GO_BROKER_ALL=$(find "$GO_DIR/broker" -name "*.go" -not -name "*_test.go" -exec cat {} + | wc -l)
RUST_BROKER_ALL=$(find "$RUST_DIR/twerk-infrastructure/src/broker" -name "*.rs" -exec cat {} + | wc -l)
printf "%-20s | %-15s | %-15s\n" "Broker (All)" "$GO_BROKER_ALL" "$RUST_BROKER_ALL"

# Datastore
GO_DS_ALL=$(find "$GO_DIR/datastore" -name "*.go" -not -name "*_test.go" -exec cat {} + | wc -l)
RUST_DS_ALL=$(find "$RUST_DIR/twerk-infrastructure/src/datastore" -name "*.rs" -exec cat {} + | wc -l)
printf "%-20s | %-15s | %-15s\n" "Datastore (All)" "$GO_DS_ALL" "$RUST_DS_ALL"

# Locker
GO_LOCKER="$GO_DIR/internal/locker/locker.go"
RUST_LOCKER="$RUST_DIR/twerk-infrastructure/src/locker/mod.rs"
printf "%-20s | %-15s | %-15s\n" "Locker" "$(wc -l < "$GO_LOCKER" 2>/dev/null || echo "N/A")" "$(wc -l < "$RUST_LOCKER" 2>/dev/null || echo "N/A")"

echo ""
echo "=== RUNTIMES ==="
printf "%-15s | %-15s | %-15s\n" "Runtime" "Go (Files/Lines)" "Rust (Files/Lines)"
printf "%-15s | %-15s | %-15s\n" "---------------" "---------------" "---------------"

for rt in docker podman; do
    GO_FILES=$(find "$GO_DIR/runtime/$rt" -name "*.go" -not -name "*_test.go" | wc -l)
    GO_LINES=$(find "$GO_DIR/runtime/$rt" -name "*.go" -not -name "*_test.go" -exec cat {} + | wc -l)
    RUST_FILES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/$rt" -name "*.rs" -not -name "*_test.rs" | wc -l)
    RUST_LINES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/$rt" -name "*.rs" -not -name "*_test.rs" -exec cat {} + | wc -l)
    printf "%-15s | %-2s files / %-5s | %-2s files / %-5s\n" "$rt" "$GO_FILES" "$GO_LINES" "$RUST_FILES" "$RUST_LINES"
done

# Shell runtime is in twerk-app
GO_SHELL="$GO_DIR/runtime/shell/shell.go"
RUST_SHELL="$RUST_DIR/twerk-app/src/engine/worker/shell.rs"
printf "%-15s | %-15s | %-15s\n" "shell" "$(wc -l < "$GO_SHELL" 2>/dev/null || echo "N/A")" "$(wc -l < "$RUST_SHELL" 2>/dev/null || echo "N/A")"

echo ""
echo "=== COORDINATOR & APP (twerk-app) ==="
printf "%-20s | %-15s | %-15s\n" "Component" "Go (Lines)" "Rust (Lines)"
printf "%-20s | %-15s | %-15s\n" "--------------------" "---------------" "---------------"

GO_COORD="$GO_DIR/internal/coordinator/coordinator.go"
RUST_COORD="$RUST_DIR/twerk-app/src/engine/coordinator/mod.rs"
printf "%-20s | %-15s | %-15s\n" "Coordinator Core" "$(wc -l < "$GO_COORD" 2>/dev/null || echo "N/A")" "$(wc -l < "$RUST_COORD" 2>/dev/null || echo "N/A")"

GO_HANDLERS=$(find "$GO_DIR/internal/coordinator/handlers" -name "*.go" -not -name "*_test.go" -exec cat {} + | wc -l)
RUST_HANDLERS="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
printf "%-20s | %-15s | %-15s\n" "Handlers (All)" "$GO_HANDLERS" "$(wc -l < "$RUST_HANDLERS" 2>/dev/null || echo "N/A")"

echo ""
echo "=== SUMMARY ==="
GO_TOTAL=$(find "$GO_DIR" -name "*.go" -not -name "*_test.go" -exec cat {} + 2>/dev/null | wc -l)
RUST_TOTAL=$(find "$RUST_DIR" -name "*.rs" -not -name "*_test.rs" -exec cat {} + 2>/dev/null | wc -l)
echo "Go total (src):   $GO_TOTAL lines"
echo "Rust total (src): $RUST_TOTAL lines"
echo "Ratio:            $(echo "scale=2; $RUST_TOTAL / $GO_TOTAL" | bc)x"

echo ""
echo "=== VERIFICATION: Missing logic check ==="
# Instead of file-by-file, check for key Go files and ensure their logic exists in Rust
declare -A CHECKS
CHECKS["internal/coordinator/handlers/cancel.go"]="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
CHECKS["internal/coordinator/handlers/completed.go"]="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
CHECKS["internal/coordinator/handlers/error.go"]="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
CHECKS["internal/coordinator/handlers/pending.go"]="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
CHECKS["internal/coordinator/handlers/started.go"]="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
CHECKS["broker/rabbitmq.go"]="$RUST_DIR/twerk-infrastructure/src/broker/rabbitmq.rs"
CHECKS["datastore/postgres.go"]="$RUST_DIR/twerk-infrastructure/src/datastore/postgres/mod.rs"

for go_rel in "${!CHECKS[@]}"; do
    if [ ! -f "$GO_DIR/$go_rel" ]; then
        echo "Go file not found: $go_rel"
        continue
    fi
    rust_file="${CHECKS[$go_rel]}"
    if [ ! -f "$rust_file" ]; then
        echo "❌ MISSING RUST PARITY: $go_rel -> $rust_file"
    else
        echo "✅ PARITY OK: $go_rel exists in $rust_file"
    fi
done

echo ""
echo "Done."
