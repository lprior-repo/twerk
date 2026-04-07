#!/bin/bash
# Honest throughput/latency benchmark
# Target: < 10ms latency, > 20,000 ops/sec throughput
# 
# NOTE: This now measures what it claims - file system speed, NOT YAML parsing.
# For TRUE YAML parsing benchmarks, run:
#   cargo test -p twerk-web --test yaml_parse_benchmark -- --nocapture

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "═══════════════════════════════════════════════════════════════════"
echo "          TWERK DISK I/O BENCHMARK"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "NOTE: This measures FILE SYSTEM speed, not twerk YAML parsing."
echo "For TRUE YAML parsing benchmarks, run:"
echo "  cargo test -p twerk-web --test yaml_parse_benchmark -- --nocapture"
echo ""

# Build release first
echo "[1/4] Building release..."
cargo build --release --quiet 2>/dev/null || cargo build --release
echo "      Build complete"
echo ""

# Create test YAML file
echo "[2/4] Creating test YAML workflow..."
mkdir -p /tmp/twerk_bench

cat > /tmp/twerk_bench/simple_echo.yaml << 'YAML'
name: bench-echo
version: "1.0"
tasks:
  - name: echo
    image: bash:latest
    command: ["echo", "hello world"]
YAML

echo "      Created /tmp/twerk_bench/simple_echo.yaml"
echo ""

# File I/O benchmark (what this script ACTUALLY measures)
echo "[3/4] File System I/O Benchmark..."
echo "─────────────────────────────────"

IO_COUNT=10000
IO_FILE="/tmp/twerk_bench/io_test"

start_ns=$(date +%s%N)
for i in $(seq 1 $IO_COUNT); do
    cat /tmp/twerk_bench/simple_echo.yaml > "$IO_FILE"
    cat "$IO_FILE" > /dev/null
done
end_ns=$(date +%s%N)
rm -f "$IO_FILE"

io_time=$(( (end_ns - start_ns) / 1000000 ))
io_per_sec=$(( IO_COUNT * 1000 / io_time ))

echo "  File I/O operations: $IO_COUNT"
echo "  Total time: ${io_time}ms"
echo "  Throughput: ${io_per_sec} ops/sec"
echo ""

# Latency test
echo "[4/4] File Read Latency Test..."
echo "─────────────────────────────────"

LATENCY_RUNS=1000
start_ns=$(date +%s%N)
for i in $(seq 1 $LATENCY_RUNS); do
    cat /tmp/twerk_bench/simple_echo.yaml > /dev/null
done
end_ns=$(date +%s%N)

latency_total=$(( (end_ns - start_ns) / 1000 ))  # microseconds
latency_avg=$(( latency_total / LATENCY_RUNS ))

echo "  Operations: $LATENCY_RUNS"
echo "  Average latency: ${latency_avg}µs (0.$(printf '%03d' $latency_avg))ms"
echo ""

# Summary
echo "═══════════════════════════════════════════════════════════════════"
echo "                    BENCHMARK SUMMARY"
echo "═══════════════════════════════════════════════════════════════════"
echo ""

echo "┌─────────────────────────────────────────────────────────────────┐"
echo "│ WARNING: This script measures FILE SYSTEM speed, not twerk       │"
echo "├─────────────────────────────────────────────────────────────────┤"
echo "│ For TRUE YAML parsing benchmarks, run:                           │"
echo "│   cargo test -p twerk-web --test yaml_parse_benchmark \\        │"
echo "│       -- --nocapture                                            │"
echo "└─────────────────────────────────────────────────────────────────┘"
echo ""

# File I/O results
echo "┌────────────────────┬─────────────────┬─────────────────┐"
echo "│ Metric             │ Current         │ Notes           │"
echo "├────────────────────┼─────────────────┼─────────────────┤"
printf "│ File I/O Rate      │ %12.0f/s     │ Disk speed     │\n" "$io_per_sec"
printf "│ File Read Latency  │ %12.0f µs    │ Per operation  │\n" "$latency_avg"
echo "└────────────────────┴─────────────────┴─────────────────┘"
echo ""

# Cleanup
rm -rf /tmp/twerk_bench

echo "═══════════════════════════════════════════════════════════════════"
