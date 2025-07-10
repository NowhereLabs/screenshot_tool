#!/bin/bash

# Screenshot Tool Benchmark Runner
# Provides structured benchmark execution with output file generation

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/benchmark_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

run_fast_benchmarks() {
    print_header "Running Fast Benchmarks (Unit Tests)"
    
    local output_file="$OUTPUT_DIR/fast_benchmarks_${TIMESTAMP}.json"
    local csv_file="$OUTPUT_DIR/fast_benchmarks_${TIMESTAMP}.csv"
    
    echo "Output files:"
    echo "  JSON: $output_file"
    echo "  CSV:  $csv_file"
    echo ""
    
    cd "$PROJECT_ROOT"
    
    # Run fast benchmarks with JSON output
    if cargo bench --bench fast_bench -- --output-format json > "$output_file" 2>&1; then
        print_success "Fast benchmarks completed successfully"
        
        # Convert to CSV for easier analysis
        if command -v jq &> /dev/null; then
            echo "function,mean_time_ns,std_dev_ns,measurement_count" > "$csv_file"
            jq -r '.results[] | select(.criterion_benchmark_v1) | 
                   [.criterion_benchmark_v1.function_id, 
                    .criterion_benchmark_v1.mean.estimate, 
                    .criterion_benchmark_v1.std_dev.estimate,
                    .criterion_benchmark_v1.measurement_count] | 
                   @csv' "$output_file" >> "$csv_file" 2>/dev/null || true
            print_success "CSV output generated"
        else
            print_warning "jq not found, skipping CSV generation"
        fi
    else
        print_error "Fast benchmarks failed"
        cat "$output_file"
        return 1
    fi
}

run_slow_benchmarks() {
    print_header "Running Slow Benchmarks (Integration Tests)"
    
    local output_file="$OUTPUT_DIR/slow_benchmarks_${TIMESTAMP}.json"
    local csv_file="$OUTPUT_DIR/slow_benchmarks_${TIMESTAMP}.csv"
    
    echo "Output files:"
    echo "  JSON: $output_file"
    echo "  CSV:  $csv_file"
    echo ""
    
    cd "$PROJECT_ROOT"
    
    # Check if Chrome is available
    if ! command -v google-chrome &> /dev/null && ! command -v chromium &> /dev/null; then
        print_warning "Chrome/Chromium not found, slow benchmarks may fail"
    fi
    
    # Run slow benchmarks with JSON output
    if timeout 600 cargo bench --bench slow_bench -- --output-format json > "$output_file" 2>&1; then
        print_success "Slow benchmarks completed successfully"
        
        # Convert to CSV for easier analysis
        if command -v jq &> /dev/null; then
            echo "function,mean_time_ns,std_dev_ns,measurement_count" > "$csv_file"
            jq -r '.results[] | select(.criterion_benchmark_v1) | 
                   [.criterion_benchmark_v1.function_id, 
                    .criterion_benchmark_v1.mean.estimate, 
                    .criterion_benchmark_v1.std_dev.estimate,
                    .criterion_benchmark_v1.measurement_count] | 
                   @csv' "$output_file" >> "$csv_file" 2>/dev/null || true
            print_success "CSV output generated"
        else
            print_warning "jq not found, skipping CSV generation"
        fi
    else
        print_error "Slow benchmarks failed or timed out"
        cat "$output_file"
        return 1
    fi
}

run_timeout_benchmarks() {
    print_header "Running Timeout Benchmarks"
    
    local output_file="$OUTPUT_DIR/timeout_benchmarks_${TIMESTAMP}.json"
    local csv_file="$OUTPUT_DIR/timeout_benchmarks_${TIMESTAMP}.csv"
    
    echo "Output files:"
    echo "  JSON: $output_file"
    echo "  CSV:  $csv_file"
    echo ""
    
    cd "$PROJECT_ROOT"
    
    # Run timeout benchmarks with JSON output
    if timeout 900 cargo bench --bench timeout_bench -- --output-format json > "$output_file" 2>&1; then
        print_success "Timeout benchmarks completed successfully"
        
        # Convert to CSV for easier analysis
        if command -v jq &> /dev/null; then
            echo "function,mean_time_ns,std_dev_ns,measurement_count" > "$csv_file"
            jq -r '.results[] | select(.criterion_benchmark_v1) | 
                   [.criterion_benchmark_v1.function_id, 
                    .criterion_benchmark_v1.mean.estimate, 
                    .criterion_benchmark_v1.std_dev.estimate,
                    .criterion_benchmark_v1.measurement_count] | 
                   @csv' "$output_file" >> "$csv_file" 2>/dev/null || true
            print_success "CSV output generated"
        else
            print_warning "jq not found, skipping CSV generation"
        fi
    else
        print_error "Timeout benchmarks failed or timed out"
        cat "$output_file"
        return 1
    fi
}

generate_summary_report() {
    print_header "Generating Summary Report"
    
    local report_file="$OUTPUT_DIR/benchmark_summary_${TIMESTAMP}.md"
    
    cat > "$report_file" << EOF
# Benchmark Summary Report

Generated: $(date)
Timestamp: $TIMESTAMP

## Test Environment

- OS: $(uname -s)
- Architecture: $(uname -m)
- Rust Version: $(rustc --version)
- Chrome Available: $(if command -v google-chrome &> /dev/null || command -v chromium &> /dev/null; then echo "Yes"; else echo "No"; fi)

## Results

### Fast Benchmarks (Unit Tests)
$(if [ -f "$OUTPUT_DIR/fast_benchmarks_${TIMESTAMP}.json" ]; then echo "✓ Completed"; else echo "✗ Failed"; fi)

### Slow Benchmarks (Integration Tests)  
$(if [ -f "$OUTPUT_DIR/slow_benchmarks_${TIMESTAMP}.json" ]; then echo "✓ Completed"; else echo "✗ Failed"; fi)

### Timeout Benchmarks
$(if [ -f "$OUTPUT_DIR/timeout_benchmarks_${TIMESTAMP}.json" ]; then echo "✓ Completed"; else echo "✗ Failed"; fi)

## Output Files

- Fast Benchmarks: \`benchmark_results/fast_benchmarks_${TIMESTAMP}.json\`
- Slow Benchmarks: \`benchmark_results/slow_benchmarks_${TIMESTAMP}.json\`
- Timeout Benchmarks: \`benchmark_results/timeout_benchmarks_${TIMESTAMP}.json\`

## Usage

To run individual benchmark suites:

\`\`\`bash
# Fast benchmarks only (10-30 seconds)
cargo bench --bench fast_bench

# Slow benchmarks only (2-5 minutes)
cargo bench --bench slow_bench

# Timeout benchmarks only (5-10 minutes)
cargo bench --bench timeout_bench

# All benchmarks
./scripts/run_benchmarks.sh all
\`\`\`

## Analysis

Use the generated CSV files for analysis:

\`\`\`bash
# View fast benchmark results
column -t -s, benchmark_results/fast_benchmarks_${TIMESTAMP}.csv

# Compare with previous results
diff benchmark_results/fast_benchmarks_\${OLD_TIMESTAMP}.csv benchmark_results/fast_benchmarks_${TIMESTAMP}.csv
\`\`\`
EOF

    print_success "Summary report generated: $report_file"
}

# Main execution
case "${1:-all}" in
    "fast")
        run_fast_benchmarks
        ;;
    "slow")
        run_slow_benchmarks
        ;;
    "timeout")
        run_timeout_benchmarks
        ;;
    "all")
        run_fast_benchmarks
        echo ""
        run_slow_benchmarks
        echo ""
        run_timeout_benchmarks
        echo ""
        generate_summary_report
        ;;
    *)
        echo "Usage: $0 [fast|slow|timeout|all]"
        echo ""
        echo "  fast    - Run fast benchmarks (unit tests, ~30 seconds)"
        echo "  slow    - Run slow benchmarks (integration tests, ~5 minutes)"  
        echo "  timeout - Run timeout benchmarks (~10 minutes)"
        echo "  all     - Run all benchmarks (default)"
        exit 1
        ;;
esac

print_header "Benchmark Results Location"
echo "All results saved to: $OUTPUT_DIR"
echo "Timestamp: $TIMESTAMP"