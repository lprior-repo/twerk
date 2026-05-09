#!/bin/bash
# Script to count Go vs Rust test functions

echo "=============================================="
echo "  GO vs RUST TEST COUNT COMPARISON"
echo "=============================================="
echo ""

echo "=== GO TEST FUNCTIONS (by file) ==="
find /tmp/tork -name "*_test.go" -type f -exec grep -l 'func Test' {} \; 2>/dev/null | sort | while read file; do
    count=$(grep -o 'func Test' "$file" | wc -l)
    echo "  $(basename "$file"): $count"
done

total_go=$(find /tmp/tork -name "*_test.go" -type f -exec grep -o 'func Test' {} \; 2>/dev/null | wc -l)
echo "---"
echo "TOTAL GO TESTS: $total_go"
echo ""

echo "=== RUST TEST FUNCTIONS (top 25 by count, excluding target/) ==="
find /home/lewis/src/twerk -name "*.rs" -type f -not -path "*/target/*" 2>/dev/null | while read file; do
    count=$(grep -o '#\[test\]' "$file" | wc -l)
    if [ "$count" -gt 0 ]; then
        echo "$count $file"
    fi
done | sort -rn | head -25 | while read count file; do
    echo "  $(basename "$file"): $count"
done

total_rust=$(find /home/lewis/src/twerk -name "*.rs" -type f -not -path "*/target/*" -exec grep -o '#\[test\]' {} \; 2>/dev/null | wc -l)
echo "---"
echo "TOTAL RUST TESTS (excluding target/): $total_rust"
echo ""

echo "=============================================="
echo "  SUMMARY"
echo "=============================================="
echo "Go tests:  $total_go"
echo "Rust tests: $total_rust"
if [ "$total_go" -gt 0 ]; then
    ratio=$(awk "BEGIN {printf \"%.2f\", $total_rust / $total_go}")
    echo "Ratio Rust/Go: $ratio"
fi