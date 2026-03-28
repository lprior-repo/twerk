#!/bin/bash
# compare_go_rust.sh - Line-by-line comparison of Go Tork vs Rust Twerk

set -e

GO_DIR="/tmp/tork"
RUST_DIR="/home/lewis/src/twerk/crates"

echo "=============================================="
echo "  GO TORK vs RUST TWERK - LINE COMPARISON"
echo "=============================================="

echo ""
echo "=== CORE FILES ==="
for file in job task node user role mount; do
    GO_FILE="$GO_DIR/${file}.go"
    RUST_FILE="$RUST_DIR/twerk-core/src/${file}.rs"

    echo ""
    echo "--- $file ---"
    if [ -f "$GO_FILE" ]; then
        GO_LINES=$(wc -l < "$GO_FILE")
        GO_FUNCS=$(grep -c "^func " "$GO_FILE" || true)
        echo "Go: $GO_LINES lines, $GO_FUNCS funcs"
    else
        echo "Go: NOT FOUND"
    fi

    if [ -f "$RUST_FILE" ]; then
        RUST_LINES=$(wc -l < "$RUST_FILE")
        RUST_FUNCS=$(grep -c "^pub fn \|^fn " "$RUST_FILE" || true)
        echo "Rust: $RUST_LINES lines, $RUST_FUNCS funcs"
    else
        echo "Rust: NOT FOUND"
    fi
done

echo ""
echo "=== INTERNAL PACKAGES ==="
for pkg in eval webhook uuid locker; do
    GO_FILE="$GO_DIR/internal/$pkg/${pkg}.go"
    if [ "$pkg" = "locker" ]; then
        RUST_DIR_PKG="$RUST_DIR/twerk-infrastructure/src/locker/mod.rs"
    else
        RUST_DIR_PKG="$RUST_DIR/twerk-core/src/${pkg}.rs"
    fi

    echo ""
    echo "--- $pkg ---"
    if [ -f "$GO_FILE" ]; then
        GO_LINES=$(wc -l < "$GO_FILE")
        echo "Go: $GO_LINES lines"
    else
        echo "Go: NOT FOUND"
    fi

    if [ -f "$RUST_DIR_PKG" ]; then
        RUST_LINES=$(wc -l < "$RUST_DIR_PKG")
        echo "Rust: $RUST_LINES lines"
    else
        echo "Rust: NOT FOUND"
    fi
done

echo ""
echo "=== BROKER ==="
GO_FILE="$GO_DIR/broker/broker.go"
RUST_FILE="$RUST_DIR/twerk-infrastructure/src/broker/mod.rs"
echo "--- broker trait ---"
[ -f "$GO_FILE" ] && echo "Go: $(wc -l < "$GO_FILE") lines" || echo "Go: NOT FOUND"
[ -f "$RUST_FILE" ] && echo "Rust: $(wc -l < "$RUST_FILE") lines" || echo "Rust: NOT FOUND"

echo ""
echo "--- broker/inmemory ---"
GO_FILE="$GO_DIR/broker/inmemory.go"
RUST_FILE="$RUST_DIR/twerk-infrastructure/src/broker/inmemory.rs"
[ -f "$GO_FILE" ] && echo "Go: $(wc -l < "$GO_FILE") lines" || echo "Go: NOT FOUND"
[ -f "$RUST_FILE" ] && echo "Rust: $(wc -l < "$RUST_FILE") lines" || echo "Rust: NOT FOUND"

echo ""
echo "=== COORDINATOR ==="
GO_FILE="$GO_DIR/internal/coordinator/coordinator.go"
RUST_FILE="$RUST_DIR/twerk-app/src/engine/coordinator/mod.rs"
echo "--- coordinator ---"
[ -f "$GO_FILE" ] && echo "Go: $(wc -l < "$GO_FILE") lines" || echo "Go: NOT FOUND"
[ -f "$RUST_FILE" ] && echo "Rust: $(wc -l < "$RUST_FILE") lines" || echo "Rust: NOT FOUND"

echo ""
echo "--- handlers ---"
GO_FILE="$GO_DIR/internal/coordinator/handlers/"
RUST_FILE="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
if [ -d "$GO_FILE" ]; then
    GO_LINES=$(find "$GO_FILE" -name "*.go" -exec wc -l {} + | tail -1 | awk '{print $1}')
    echo "Go: $GO_LINES lines"
else
    echo "Go: NOT FOUND"
fi
[ -f "$RUST_FILE" ] && echo "Rust: $(wc -l < "$RUST_FILE") lines" || echo "Rust: NOT FOUND"

echo ""
echo "=== RUNTIME ==="
echo "--- docker ---"
GO_FILES=$(find "$GO_DIR/runtime/docker" -name "*.go" 2>/dev/null | wc -l)
GO_LINES=$(find "$GO_DIR/runtime/docker" -name "*.go" -exec cat {} + 2>/dev/null | wc -l)
echo "Go: $GO_FILES files, $GO_LINES lines"
RUST_FILES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/docker" -name "*.rs" 2>/dev/null | wc -l)
RUST_LINES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/docker" -name "*.rs" -exec cat {} + 2>/dev/null | wc -l)
echo "Rust: $RUST_FILES files, $RUST_LINES lines"

echo ""
echo "--- podman ---"
GO_FILES=$(find "$GO_DIR/runtime/podman" -name "*.go" 2>/dev/null | wc -l)
GO_LINES=$(find "$GO_DIR/runtime/podman" -name "*.go" -exec cat {} + 2>/dev/null | wc -l)
echo "Go: $GO_FILES files, $GO_LINES lines"
RUST_FILES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/podman" -name "*.rs" 2>/dev/null | wc -l)
RUST_LINES=$(find "$RUST_DIR/twerk-infrastructure/src/runtime/podman" -name "*.rs" -exec cat {} + 2>/dev/null | wc -l)
echo "Rust: $RUST_FILES files, $RUST_LINES lines"

echo ""
echo "--- shell ---"
GO_FILE="$GO_DIR/runtime/shell/shell.go"
RUST_FILE="$RUST_DIR/twerk-app/src/engine/worker/shell.rs"
[ -f "$GO_FILE" ] && echo "Go: $(wc -l < "$GO_FILE") lines" || echo "Go: NOT FOUND"
[ -f "$RUST_FILE" ] && echo "Rust: $(wc -l < "$RUST_FILE") lines" || echo "Rust: NOT FOUND"

echo ""
echo "=============================================="
echo "  TOTALS"
echo "=============================================="
echo ""
GO_TOTAL=$(find "$GO_DIR" -name "*.go" -exec cat {} + 2>/dev/null | wc -l)
echo "Go total: $GO_TOTAL lines"
RUST_TOTAL=$(find "$RUST_DIR" -name "*.rs" -exec cat {} + 2>/dev/null | wc -l)
echo "Rust total: $RUST_TOTAL lines"
echo "Ratio: $(echo "scale:2; $RUST_TOTAL / $GO_TOTAL" | bc)x"

echo ""
echo "=== MISSING FILES CHECK ==="
echo "Checking for Go files without Rust equivalents..."
for go_file in $(find "$GO_DIR" -name "*.go" | grep -v "_test.go" | grep -v "setid_unix" | grep -v "setid_unsupported"); do
    base=$(basename "$go_file" .go)
    go_dir=$(dirname "$go_file")
    pkg=$(basename "$go_dir")

    # Map Go package to Rust location
    case "$pkg" in
        internal/coordinator/handlers)
            rust_path="$RUST_DIR/twerk-app/src/engine/coordinator/handlers.rs"
            ;;
        broker)
            rust_path="$RUST_DIR/twerk-infrastructure/src/broker/${base}.rs"
            ;;
        runtime/docker)
            rust_path="$RUST_DIR/twerk-infrastructure/src/runtime/docker/${base}.rs"
            ;;
        runtime/podman)
            rust_path="$RUST_DIR/twerk-infrastructure/src/runtime/podman/${base}.rs"
            ;;
        runtime/shell)
            rust_path="$RUST_DIR/twerk-app/src/engine/worker/shell.rs"
            ;;
        internal)
            rust_path=""
            ;;
        *)
            rust_path="$RUST_DIR/twerk-core/src/${base}.rs"
            ;;
    esac

    if [ -n "$rust_path" ] && [ ! -f "$rust_path" ]; then
        echo "MISSING: $go_file -> $rust_path"
    fi
done

echo ""
echo "Done."