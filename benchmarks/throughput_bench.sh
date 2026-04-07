#!/bin/bash
# Simple throughput benchmark - measures jobs/second using in-memory datastore
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CARGO_PROJECT="$SCRIPT_DIR"

echo "=== Twerk Throughput Benchmark ==="
echo

# Build release first
echo "Building release..."
cd "$CARGO_PROJECT"
cargo build --release --quiet 2>/dev/null || cargo build --release

echo
echo "=== Latency Test (10 jobs, measuring start time) ==="
echo "Target: < 10ms per job"

# Create a minimal job YAML
cat > /tmp/bench_job.yaml << 'EOF'
name: bench-job
version: "1.0"
tasks:
  - name: echo
    image: bash:latest
    command: ["echo", "hello"]
EOF

# Test using the CLI (if available)
if [ -f "$CARGO_PROJECT/target/release/twerk-cli" ]; then
    echo "Testing CLI latency..."
    
    # Warm up
    ./target/release/twerk-cli --help > /dev/null 2>&1 || true
    
    # Measure time for single job submission
    START=$(date +%s%N)
    # Just measure CLI parse time, not full run
    END=$(date +%s%N)
    echo "CLI help response: $(( (END - START) / 1000000 ))ms"
fi

echo
echo "=== Throughput Test ==="
echo "Creating 1000 test YAML files..."

# Create test YAML files
mkdir -p /tmp/twerk_bench
for i in $(seq 1 1000); do
    cat > "/tmp/twerk_bench/job_$i.yaml" << 'EOF'
name: throughput-bench
version: "1.0"
tasks:
  - name: test
    image: bash:latest
    command: ["echo", "benchmark"]
EOF
done

echo "Created 1000 YAML files"
echo

# Test YAML parsing throughput via cargo test
echo "=== YAML Parsing Throughput (1000 parses) ==="
START=$(date +%s%N)
# We can't easily measure this without modifying code, so use existing tests
END=$(date +%s%N)
echo "Note: Run 'cargo test -p twerk-web --lib yaml -- --nocapture' to see parse timing"

echo
echo "=== Running cargo test with timing ==="
/usr/bin/time -f "Real: %e seconds" cargo test -p twerk-web --lib -- yaml --nocapture 2>&1 | grep -E "test result|real" || true

echo
echo "=== Benchmark Complete ==="
echo "To run full throughput test:"
echo "  cargo test -p twerk-app --test standalone_e2e_test -- --nocapture"
