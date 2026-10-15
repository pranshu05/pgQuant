#!/usr/bin/env bash
# =============================================================================
# pgQuant Automated Benchmark Pipeline
# =============================================================================
# Discovers ALL functions in the pgquant schema via pg_proc, infers argument
# types, generates safe placeholder arguments, and runs EXPLAIN (ANALYZE,
# BUFFERS) for each.  Also runs the sql_benchmark binary and optional
# criterion benches, then consolidates everything into benchmark_results/.
# =============================================================================
set -euo pipefail

# ── configuration ───────────────────────────────────────────────────────────
RESULT_DIR="benchmark_results"
PG_VER="${PG_VER:-pg17}"               # override via env for CI matrix
NUM_ROWS="${NUM_ROWS:-10000}"           # synthetic data row count
PSQL_CONN=""                           # populated after pgrx start

# ── helpers ─────────────────────────────────────────────────────────────────
log()  { printf '\033[1;34m▸ %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m⚠ %s\033[0m\n' "$*"; }
fail() { printf '\033[1;31m✖ %s\033[0m\n' "$*"; exit 1; }

cleanup() {
    log "Cleaning up …"
    cargo pgrx stop "$PG_VER" 2>/dev/null || true
    rm -rf target/criterion
}
trap cleanup EXIT

# ── 0. prepare result directory ─────────────────────────────────────────────
rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

# ── 1. run the SQL-level benchmark binary ───────────────────────────────────
BIN="./target/release/sql_benchmark"
SQL_LOG="$RESULT_DIR/sql_benchmark.log"

if [[ -x "$BIN" ]]; then
    log "Running SQL benchmark binary …"
    "$BIN" 2>&1 | tee "$SQL_LOG" || {
        warn "sql_benchmark exited non-zero; output captured"
    }
else
    warn "Binary $BIN not found – skipping SQL benchmark"
    echo "(sql_benchmark binary was not built)" > "$SQL_LOG"
fi

# ── 2. start a temporary PostgreSQL via pgrx ────────────────────────────────
log "Starting PostgreSQL ($PG_VER) via cargo pgrx …"
cargo pgrx start "$PG_VER"
sleep 3

# Resolve the pgrx data directory to find the socket & port
PGRX_HOME="${PGRX_HOME:-$HOME/.pgrx}"
PG_NUM="${PG_VER//pg/}"

# Try to find the correct port (pgrx uses 288xx by default)
PG_PORT="288${PG_NUM}"

# Find the unix socket dir – pgrx puts it under its data dir
SOCKET_DIR="$PGRX_HOME/data-$PG_NUM"
if [[ -S "$SOCKET_DIR/.s.PGSQL.$PG_PORT" ]]; then
    PSQL_CONN="-h $SOCKET_DIR -p $PG_PORT"
elif [[ -S "/tmp/.s.PGSQL.$PG_PORT" ]]; then
    PSQL_CONN="-h /tmp -p $PG_PORT"
else
    # Fallback: try localhost TCP
    PSQL_CONN="-h localhost -p $PG_PORT"
fi

# Convenience wrapper – use the pgrx-managed postgres user
run_sql() { psql $PSQL_CONN -U "$USER" -d postgres -v ON_ERROR_STOP=1 "$@"; }

# Verify the connection works
run_sql -c "SELECT 1;" >/dev/null 2>&1 || fail "Cannot connect to PostgreSQL"
log "Connected to PostgreSQL on port $PG_PORT"

# ── 3. load extension & generate synthetic benchmark data ───────────────────
log "Loading pgquant extension & creating synthetic tables ($NUM_ROWS rows) …"
run_sql <<SQL
-- Load the extension
CREATE EXTENSION IF NOT EXISTS pgquant;

-- ────────────────────────────────────────────────────────────────
-- Table: bench_prices  (used by many scalar/timeseries functions)
-- ────────────────────────────────────────────────────────────────
DROP TABLE IF EXISTS bench_prices CASCADE;
CREATE TABLE bench_prices (
    symbol    TEXT            NOT NULL,
    date      DATE            NOT NULL,
    price     DOUBLE PRECISION NOT NULL,
    volume    BIGINT
);

INSERT INTO bench_prices (symbol, date, price, volume)
SELECT
    sym,
    '2020-01-02'::date + (i % 2000) * INTERVAL '1 day',
    50.0 + 50.0 * random() + (i % 2000)::float * 0.01,
    (random() * 1000000)::bigint
FROM
    generate_series(1, $NUM_ROWS) AS s(i),
    unnest(ARRAY['AAPL','MSFT','GOOG','AMZN','META']) AS sym
ORDER BY sym, date;

-- ────────────────────────────────────────────────────────────────
-- Table: bench_returns  (symbol, date, ret) — for covariance/vol
-- ────────────────────────────────────────────────────────────────
DROP TABLE IF EXISTS bench_returns CASCADE;
CREATE TABLE bench_returns (
    symbol  TEXT             NOT NULL,
    date    DATE             NOT NULL,
    ret     DOUBLE PRECISION NOT NULL
);

INSERT INTO bench_returns (symbol, date, ret)
SELECT
    sym,
    '2020-01-02'::date + (i % 2000) * INTERVAL '1 day',
    (random() - 0.5) * 0.1     -- daily returns in [-0.05, 0.05]
FROM
    generate_series(1, $NUM_ROWS) AS s(i),
    unnest(ARRAY['AAPL','MSFT','GOOG','AMZN','META']) AS sym
ORDER BY sym, date;

-- ────────────────────────────────────────────────────────────────
-- Table: bench_factor_panel  (for Fama-French factor functions)
-- ────────────────────────────────────────────────────────────────
DROP TABLE IF EXISTS bench_factor_panel CASCADE;
CREATE TABLE bench_factor_panel (
    symbol      TEXT             NOT NULL,
    date        DATE             NOT NULL,
    ret         DOUBLE PRECISION NOT NULL,
    size_bucket INT              NOT NULL,
    btm_bucket  INT              NOT NULL,
    weight      DOUBLE PRECISION NOT NULL DEFAULT 1.0
);

INSERT INTO bench_factor_panel (symbol, date, ret, size_bucket, btm_bucket, weight)
SELECT
    'SYM' || (i % 50),
    '2020-01-02'::date + (i % 200) * INTERVAL '1 day',
    (random() - 0.5) * 0.1,
    1 + (i % 2),          -- size: 1 or 2
    1 + (i % 3),          -- btm:  1, 2, or 3
    0.5 + random()
FROM generate_series(1, $NUM_ROWS) AS s(i);

ANALYZE bench_prices;
ANALYZE bench_returns;
ANALYZE bench_factor_panel;
SQL

log "Synthetic data inserted"

# ── 4. Dynamic function discovery & benchmarking ────────────────────────────
DYNAMIC_LOG="$RESULT_DIR/dynamic_functions.log"
: > "$DYNAMIC_LOG"

log "Discovering all functions in pgquant schema via pg_proc …"

# =============================================================================
# PLACEHOLDER ARGUMENT GENERATION
# =============================================================================
# The function below maps PostgreSQL type OIDs/names to safe literal values
# that will not crash the extension.  To extend for a new type, simply add a
# new pattern to the case statement.
#
# How it works:
#   1. We query pg_proc + pg_type to get the ordered list of argument type
#      names for every function in the pgquant schema.
#   2. For each argument type we produce a syntactically-valid SQL literal via
#      the type_to_placeholder() function.
#   3. We build a SELECT <fn>(arg1, arg2, …) statement and wrap it in
#      EXPLAIN (ANALYZE, BUFFERS) to measure planning + execution time with
#      buffer statistics.
# =============================================================================

type_to_placeholder() {
    local t="$1"
    case "$t" in
        # ── numeric scalars ──────────────────────────────────────────────
        float8|"double precision")  echo "0.01::float8" ;;
        float4|real)                echo "0.01::float4" ;;
        numeric)                    echo "0.01::numeric" ;;
        int4|integer)               echo "252" ;;
        int8|bigint)                echo "1000::bigint" ;;
        int2|smallint)              echo "10::smallint" ;;

        # ── text / queries ───────────────────────────────────────────────
        text|varchar|"character varying")
            echo "'SELECT symbol, date, ret FROM bench_returns ORDER BY symbol, date'::text" ;;
        name)                       echo "'bench_test'::name" ;;

        # ── dates / timestamps ───────────────────────────────────────────
        date)                       echo "'2024-01-01'::date" ;;
        timestamp|"timestamp without time zone")
            echo "'2024-01-01 00:00:00'::timestamp" ;;
        timestamptz|"timestamp with time zone")
            echo "'2024-01-01T00:00:00Z'::timestamptz" ;;

        # ── boolean ──────────────────────────────────────────────────────
        bool|boolean)               echo "true" ;;

        # ── arrays (common in pgquant) ───────────────────────────────────
        "_float8"|"double precision[]"|"ARRAY")
            # Generate a realistic returns array via sub-select
            echo "ARRAY(SELECT (random()-0.5)*0.1 FROM generate_series(1,100))::float8[]" ;;
        "_float4"|"real[]")
            echo "ARRAY(SELECT (random()-0.5)::float4 FROM generate_series(1,50))::float4[]" ;;
        "_int4"|"integer[]")
            echo "ARRAY[1,2,3,4,5]::int4[]" ;;
        "_text"|"text[]")
            echo "ARRAY['AAPL','MSFT','GOOG']::text[]" ;;

        # ── catch-all ────────────────────────────────────────────────────
        *)
            echo "NULL::$t" ;;
    esac
}

# Fetch all functions, their argument types, and count
FUNC_META=$(run_sql -At -F'|' <<'METASQL'
SELECT
    p.proname,
    p.pronargs,
    COALESCE(
        array_to_string(
            ARRAY(
                SELECT t.typname
                FROM unnest(p.proargtypes) WITH ORDINALITY AS a(oid, ord)
                JOIN pg_type t ON t.oid = a.oid
                ORDER BY a.ord
            ),
            ','
        ),
        ''
    ) AS argtypes
FROM pg_proc p
JOIN pg_namespace n ON p.pronamespace = n.oid
WHERE n.nspname = 'pgquant'
ORDER BY p.proname;
METASQL
)

if [[ -z "$FUNC_META" ]]; then
    warn "No functions found in pgquant schema – extension may not be loaded"
    echo "No functions discovered." > "$DYNAMIC_LOG"
else
    TOTAL=$(echo "$FUNC_META" | wc -l)
    log "Found $TOTAL functions – benchmarking each with EXPLAIN (ANALYZE, BUFFERS) …"

    IDX=0
    while IFS='|' read -r fn_name fn_nargs fn_argtypes; do
        IDX=$((IDX + 1))
        printf '\r  [%d/%d] %-50s' "$IDX" "$TOTAL" "$fn_name"

        {
            echo "═══════════════════════════════════════════════════════════════"
            echo "Function: $fn_name  (args: $fn_nargs)  types: [$fn_argtypes]"
            echo "═══════════════════════════════════════════════════════════════"
        } >> "$DYNAMIC_LOG"

        # Build argument list
        ARGS=""
        if [[ "$fn_nargs" -gt 0 && -n "$fn_argtypes" ]]; then
            IFS=',' read -ra TYPES <<< "$fn_argtypes"
            for i in "${!TYPES[@]}"; do
                PH=$(type_to_placeholder "${TYPES[$i]}")
                if [[ $i -gt 0 ]]; then
                    ARGS="$ARGS, $PH"
                else
                    ARGS="$PH"
                fi
            done
        fi

        # Run EXPLAIN (ANALYZE, BUFFERS)
        QUERY="EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) SELECT * FROM $fn_name($ARGS);"
        {
            echo "Query: SELECT * FROM $fn_name($ARGS)"
            echo "---"
            run_sql -c "$QUERY" 2>&1 || echo "(ERROR – function may require SPI context or special data)"
            echo ""
        } >> "$DYNAMIC_LOG"

    done <<< "$FUNC_META"
    printf '\n'

    log "Dynamic function benchmarking complete"
fi

# ── 5. Criterion benchmarks (optional) ──────────────────────────────────────
CRITERION_LOG="$RESULT_DIR/criterion.log"
rm -rf target/criterion

# Only run criterion if there are bench targets registered in Cargo.toml
if grep -q '^\[\[bench\]\]' Cargo.toml 2>/dev/null; then
    log "Running criterion benchmarks …"
    cargo bench --quiet 2>&1 | tee "$CRITERION_LOG" || {
        warn "Criterion benchmarks exited non-zero"
    }
else
    log "No [[bench]] targets in Cargo.toml – skipping criterion"
    echo "(No criterion bench targets configured)" > "$CRITERION_LOG"
fi

# ── 6. Assemble markdown report ────────────────────────────────────────────
log "Generating benchmark report …"
REPORT="$RESULT_DIR/README.md"
{
    cat <<'HEADER'
# pgQuant Benchmark Report

> Auto-generated by `scripts/run_benchmarks.sh`

HEADER

    echo "**Date:** $(date -u +'%Y-%m-%d %H:%M:%S UTC')"
    echo "**PostgreSQL:** $PG_VER"
    echo "**Synthetic rows per table:** $NUM_ROWS"
    echo ""

    # ── SQL binary ──────────────────────────────────────────────────────
    echo "## 1. SQL-level Benchmark (binary)"
    echo ""
    echo '```text'
    cat "$SQL_LOG"
    echo '```'
    echo ""

    # ── Dynamic discovery ───────────────────────────────────────────────
    echo "## 2. Dynamic Function Benchmarks"
    echo ""
    echo "Every function in the \`pgquant\` schema was discovered via \`pg_proc\`,"
    echo "placeholder arguments were generated from the argument type OIDs, and"
    echo "each was executed with \`EXPLAIN (ANALYZE, BUFFERS)\`."
    echo ""
    echo '```text'
    cat "$DYNAMIC_LOG"
    echo '```'
    echo ""

    # ── Criterion ───────────────────────────────────────────────────────
    echo "## 3. Criterion (Rust micro-benchmarks)"
    echo ""
    echo '```text'
    cat "$CRITERION_LOG"
    echo '```'
    echo ""

    # ── Footer ──────────────────────────────────────────────────────────
    echo "---"
    echo ""
    echo "### Extending Placeholder Arguments"
    echo ""
    echo "The \`type_to_placeholder()\` function in \`scripts/run_benchmarks.sh\`"
    echo "maps PostgreSQL type names to safe SQL literals.  To support a new type:"
    echo ""
    echo "1. Open \`scripts/run_benchmarks.sh\`"
    echo "2. Add a new pattern to the \`case\` statement inside \`type_to_placeholder()\`"
    echo "3. The pattern matches the \`pg_type.typname\` value (e.g. \`jsonb\`, \`uuid\`)"
    echo "4. Return a syntactically-valid SQL literal expression"
    echo ""
    echo '```bash'
    echo '# Example: adding jsonb support'
    echo "jsonb)  echo \"'{\"key\": \"value\"}'::jsonb\" ;;"
    echo '```'

} > "$REPORT"

log "Report written to $REPORT"
log "Done ✓"
