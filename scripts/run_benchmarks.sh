#!/usr/bin/env bash
set -euo pipefail

# Directory for benchmark results
RESULT_DIR="benchmark_results"
mkdir -p "$RESULT_DIR"
# Remove any previous benchmark files
rm -f "$RESULT_DIR"/*

# ------------------------------------------------------------
# 1️⃣ Run the SQL‑level benchmark (full PostgreSQL round‑trip)
# ------------------------------------------------------------
BIN="./target/release/sql_benchmark"
if [[ ! -x "$BIN" ]]; then
  echo "ERROR: Benchmark binary not found at $BIN"
  exit 1
fi

echo "Running SQL benchmark..."
"$BIN" | tee "$RESULT_DIR/sql_benchmark.log"

# ------------------------------------------------------------
# 2️⃣ Run the micro‑benchmarks (criterion) for the core Rust code
# ------------------------------------------------------------
# Clean any previous criterion results
rm -rf target/criterion


# ------------------------------------------------------------
# 3️⃣ Assemble a markdown report
# ------------------------------------------------------------
REPORT="$RESULT_DIR/README.md"
{
  echo "# Benchmark Report"
  echo
  echo "Generated on $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
  echo
  echo "## SQL‑level Benchmark"
  echo
  echo '```text'
  cat "$RESULT_DIR/sql_benchmark.log"
  echo '```'
  echo
  echo "## Criterion (Rust core) Benchmarks"
  echo
  echo '```text'
  cat "$RESULT_DIR/criterion.log"
  echo '```'
} > "$REPORT"

echo "Benchmark report written to $REPORT"
