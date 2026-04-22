#!/usr/bin/env bash
# E2E test runner for gaze CLI
# Usage: ./scripts/e2e_cli.sh [--filter CLI-E2E-NNN] [--verbose]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
GAZE="$PROJECT_DIR/target/debug/gaze"
FIXTURE_DIR="$SCRIPT_DIR/fixtures"
TMP_DIR="/tmp/gaze_e2e_$$"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0
TOTAL=0
FILTER=""
VERBOSE=false
RESULTS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --filter) FILTER="$2"; shift 2 ;;
        --verbose) VERBOSE=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

mkdir -p "$TMP_DIR"
trap 'rm -rf "$TMP_DIR"' EXIT

# ── Helpers ──────────────────────────────────────────────────

should_run() {
    [[ -z "$FILTER" ]] || [[ "$1" == "$FILTER" ]]
}

pass() {
    PASS=$((PASS + 1))
    TOTAL=$((TOTAL + 1))
    RESULTS+=("${GREEN}PASS${NC} $1 — $2")
    printf "${GREEN}PASS${NC} %s — %s\n" "$1" "$2"
}

fail() {
    FAIL=$((FAIL + 1))
    TOTAL=$((TOTAL + 1))
    RESULTS+=("${RED}FAIL${NC} $1 — $2: $3")
    printf "${RED}FAIL${NC} %s — %s: %s\n" "$1" "$2" "$3"
}

skip() {
    SKIP=$((SKIP + 1))
    TOTAL=$((TOTAL + 1))
    RESULTS+=("${YELLOW}SKIP${NC} $1 — $2")
    printf "${YELLOW}SKIP${NC} %s — %s\n" "$1" "$2"
}

run_gaze_with_exit() {
    local stdout_file="$TMP_DIR/stdout"
    local stderr_file="$TMP_DIR/stderr"
    set +e
    "$GAZE" "$@" >"$stdout_file" 2>"$stderr_file"
    LAST_EXIT=$?
    set -e
    LAST_STDOUT=$(cat "$stdout_file")
    LAST_STDERR=$(cat "$stderr_file")
    if $VERBOSE; then
        [[ -n "$LAST_STDOUT" ]] && echo "  stdout: $LAST_STDOUT"
        [[ -n "$LAST_STDERR" ]] && echo "  stderr: $LAST_STDERR"
        echo "  exit: $LAST_EXIT"
    fi
}

assert_exit() {
    local expected=$1 id=$2 desc=$3
    if [[ "$LAST_EXIT" -ne "$expected" ]]; then
        fail "$id" "$desc" "expected exit=$expected, got=$LAST_EXIT"
        return 1
    fi
    return 0
}

assert_stdout_contains() {
    local pattern=$1 id=$2 desc=$3
    if ! echo "$LAST_STDOUT" | grep -q "$pattern"; then
        fail "$id" "$desc" "stdout missing: $pattern"
        return 1
    fi
    return 0
}

assert_stderr_contains() {
    local pattern=$1 id=$2 desc=$3
    if ! echo "$LAST_STDERR" | grep -q "$pattern"; then
        fail "$id" "$desc" "stderr missing: $pattern"
        return 1
    fi
    return 0
}

assert_json_key() {
    local key=$1 id=$2 desc=$3
    if ! echo "$LAST_STDOUT" | jq "has(\"$key\")" 2>/dev/null | grep -q true; then
        fail "$id" "$desc" "JSON key missing: $key"
        return 1
    fi
    return 0
}

assert_json_value() {
    local key=$1 expected=$2 id=$3 desc=$4
    local actual
    actual=$(echo "$LAST_STDOUT" | jq -r ".$key" 2>/dev/null)
    if [[ "$actual" != "$expected" ]]; then
        fail "$id" "$desc" ".$key: expected=$expected, got=$actual"
        return 1
    fi
    return 0
}

assert_file_exists() {
    local path=$1 id=$2 desc=$3
    if [[ ! -f "$path" ]]; then
        fail "$id" "$desc" "file not found: $path"
        return 1
    fi
    return 0
}

# Check if screen recording permission is available
check_permission() {
    run_gaze_with_exit list displays
    [[ "$LAST_EXIT" -eq 0 ]]
}

main() {

# ── Prerequisites ────────────────────────────────────────────

echo ""
printf "${CYAN}═══ Gaze CLI E2E Tests ═══${NC}\n"
echo ""

# Build check
if [[ ! -x "$GAZE" ]]; then
    echo "Binary not found. Building..."
    (cd "$PROJECT_DIR" && cargo build -p gaze-cli 2>&1)
fi

# Generate fixtures (skip if already present)
if [[ ! -f "$FIXTURE_DIR/e2e_input.png" ]]; then
    bash "$SCRIPT_DIR/gen_fixtures.sh" >/dev/null 2>&1
fi

# Permission check
HAS_PERMISSION=false
if check_permission; then
    HAS_PERMISSION=true
    printf "${GREEN}Screen recording permission: granted${NC}\n"
else
    printf "${YELLOW}Screen recording permission: denied (capture tests will skip)${NC}\n"
fi

echo ""
printf "${CYAN}── Smoke Tests ──${NC}\n"

# ── CLI-E2E-001: --help ──
if should_run "CLI-E2E-001"; then
    run_gaze_with_exit --help
    if assert_exit 0 "CLI-E2E-001" "help flag" && \
       assert_stdout_contains "capture" "CLI-E2E-001" "help flag" && \
       assert_stdout_contains "list" "CLI-E2E-001" "help flag" && \
       assert_stdout_contains "optimize" "CLI-E2E-001" "help flag" && \
       assert_stdout_contains "version" "CLI-E2E-001" "help flag"; then
        pass "CLI-E2E-001" "--help shows subcommands"
    fi
fi

# ── CLI-E2E-002: --version ──
if should_run "CLI-E2E-002"; then
    run_gaze_with_exit --version
    if assert_exit 0 "CLI-E2E-002" "--version" && \
       echo "$LAST_STDOUT" | grep -qE 'gaze [0-9]+\.[0-9]+\.[0-9]+'; then
        pass "CLI-E2E-002" "--version shows semver"
    else
        fail "CLI-E2E-002" "--version" "no semver in stdout"
    fi
fi

# ── CLI-E2E-003: version subcommand ──
if should_run "CLI-E2E-003"; then
    run_gaze_with_exit version
    if assert_exit 0 "CLI-E2E-003" "version subcommand" && \
       echo "$LAST_STDOUT" | grep -qE 'gaze [0-9]+\.[0-9]+\.[0-9]+'; then
        pass "CLI-E2E-003" "version subcommand shows semver"
    else
        fail "CLI-E2E-003" "version subcommand" "no semver in stdout"
    fi
fi

echo ""
printf "${CYAN}── Arg Validation Tests ──${NC}\n"

# ── CLI-E2E-004: invalid subcommand ──
if should_run "CLI-E2E-004"; then
    run_gaze_with_exit no-such-command
    if assert_exit 2 "CLI-E2E-004" "invalid subcommand"; then
        pass "CLI-E2E-004" "invalid subcommand → exit 2"
    fi
fi

# ── CLI-E2E-005: --format path requires --output ──
if should_run "CLI-E2E-005"; then
    run_gaze_with_exit capture --format path
    if assert_exit 2 "CLI-E2E-005" "format path w/o output" && \
       assert_stderr_contains "format path requires --output" "CLI-E2E-005" "format path w/o output"; then
        pass "CLI-E2E-005" "--format path requires --output"
    fi
fi

# ── CLI-E2E-006: --window with area mode ──
if should_run "CLI-E2E-006"; then
    run_gaze_with_exit capture --mode area --window 7
    if assert_exit 2 "CLI-E2E-006" "window+area combo" && \
       assert_stderr_contains "cannot be combined" "CLI-E2E-006" "window+area combo"; then
        pass "CLI-E2E-006" "--window + area mode → error"
    fi
fi

# ── CLI-E2E-007: --mode window --display ──
if should_run "CLI-E2E-007"; then
    run_gaze_with_exit capture --mode window --display 1
    if assert_exit 2 "CLI-E2E-007" "window mode+display" && \
       assert_stderr_contains "display" "CLI-E2E-007" "window mode+display"; then
        pass "CLI-E2E-007" "--mode window + --display → error"
    fi
fi

# ── CLI-E2E-008: --display + --window ──
if should_run "CLI-E2E-008"; then
    run_gaze_with_exit capture --display 1 --window 7
    if assert_exit 2 "CLI-E2E-008" "display+window combo" && \
       assert_stderr_contains "cannot be combined" "CLI-E2E-008" "display+window combo"; then
        pass "CLI-E2E-008" "--display + --window → error"
    fi
fi

# ── CLI-E2E-043: list without subcommand ──
if should_run "CLI-E2E-043"; then
    run_gaze_with_exit list
    if assert_exit 2 "CLI-E2E-043" "list w/o subcommand"; then
        pass "CLI-E2E-043" "list without subcommand → exit 2"
    fi
fi

echo ""
printf "${CYAN}── List Tests ──${NC}\n"

# ── CLI-E2E-011: list displays ──
if should_run "CLI-E2E-011"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-011" "list displays (no permission)"
    else
        run_gaze_with_exit list displays
        if assert_exit 0 "CLI-E2E-011" "list displays"; then
            local local_ok=true
            for key in id name width height scaleFactor; do
                if ! echo "$LAST_STDOUT" | jq ".[0] | has(\"$key\")" 2>/dev/null | grep -q true; then
                    fail "CLI-E2E-011" "list displays" "missing key: $key"
                    local_ok=false
                    break
                fi
            done
            local arr_len
            arr_len=$(echo "$LAST_STDOUT" | jq 'length' 2>/dev/null)
            if $local_ok && [[ "$arr_len" -gt 0 ]]; then
                pass "CLI-E2E-011" "list displays JSON (${arr_len} display(s))"
            elif $local_ok; then
                fail "CLI-E2E-011" "list displays" "empty array"
            fi
        fi
    fi
fi

# ── CLI-E2E-012: list windows ──
if should_run "CLI-E2E-012"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-012" "list windows (no permission)"
    else
        run_gaze_with_exit list windows
        if assert_exit 0 "CLI-E2E-012" "list windows"; then
            local local_ok=true
            for key in id title appName isOnScreen; do
                if ! echo "$LAST_STDOUT" | jq ".[0] | has(\"$key\")" 2>/dev/null | grep -q true; then
                    fail "CLI-E2E-012" "list windows" "missing key: $key"
                    local_ok=false
                    break
                fi
            done
            if $local_ok; then
                local arr_len
                arr_len=$(echo "$LAST_STDOUT" | jq 'length' 2>/dev/null)
                pass "CLI-E2E-012" "list windows JSON (${arr_len} window(s))"
            fi
        fi
    fi
fi

echo ""
printf "${CYAN}── Capture Tests ──${NC}\n"

# ── CLI-E2E-013: default capture ──
if should_run "CLI-E2E-013"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-013" "default capture (no permission)"
    else
        run_gaze_with_exit capture
        if assert_exit 0 "CLI-E2E-013" "default capture"; then
            local_ok=true
            for key in originalWidth optimizedWidth provider imageBase64; do
                if ! echo "$LAST_STDOUT" | jq -e ".$key" >/dev/null 2>&1; then
                    fail "CLI-E2E-013" "default capture" "missing key: $key"
                    local_ok=false
                    break
                fi
            done
            if $local_ok; then
                # outputPath should be absent
                if echo "$LAST_STDOUT" | jq -e '.outputPath' >/dev/null 2>&1; then
                    fail "CLI-E2E-013" "default capture" "outputPath should be absent"
                else
                    pass "CLI-E2E-013" "default capture JSON"
                fi
            fi
        fi
    fi
fi

# ── CLI-E2E-014: capture --output --format json ──
if should_run "CLI-E2E-014"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-014" "capture with output (no permission)"
    else
        local out_file="$TMP_DIR/e2e_full.webp"
        run_gaze_with_exit capture --output "$out_file" --format json
        if assert_exit 0 "CLI-E2E-014" "capture --output --format json" && \
           assert_file_exists "$out_file" "CLI-E2E-014" "capture --output" && \
           assert_json_key "outputPath" "CLI-E2E-014" "capture --output"; then
            # imageBase64 should be absent
            if echo "$LAST_STDOUT" | jq -e '.imageBase64' >/dev/null 2>&1; then
                fail "CLI-E2E-014" "capture --output" "imageBase64 should be absent"
            else
                pass "CLI-E2E-014" "capture --output writes file, omits base64"
            fi
        fi
    fi
fi

# ── CLI-E2E-015: capture --output --format path ──
if should_run "CLI-E2E-015"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-015" "capture --format path (no permission)"
    else
        local out_file="$TMP_DIR/e2e_full_path.webp"
        run_gaze_with_exit capture --output "$out_file" --format path
        if assert_exit 0 "CLI-E2E-015" "capture --format path" && \
           assert_file_exists "$out_file" "CLI-E2E-015" "capture --format path"; then
            if echo "$LAST_STDOUT" | grep -q "$out_file"; then
                pass "CLI-E2E-015" "capture --format path outputs path only"
            else
                fail "CLI-E2E-015" "capture --format path" "stdout not path"
            fi
        fi
    fi
fi

# ── CLI-E2E-016: capture --format base64 ──
if should_run "CLI-E2E-016"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-016" "capture --format base64 (no permission)"
    else
        run_gaze_with_exit capture --format base64
        if assert_exit 0 "CLI-E2E-016" "capture --format base64"; then
            if echo "$LAST_STDOUT" | grep -q '{'; then
                fail "CLI-E2E-016" "capture --format base64" "stdout contains JSON (should be raw base64)"
            elif [[ ${#LAST_STDOUT} -gt 10 ]]; then
                pass "CLI-E2E-016" "capture --format base64 outputs raw base64"
            else
                fail "CLI-E2E-016" "capture --format base64" "base64 too short"
            fi
        fi
    fi
fi

# ── CLI-E2E-017: capture --display <valid_id> ──
if should_run "CLI-E2E-017"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-017" "capture --display (no permission)"
    else
        local display_id
        display_id=$("$GAZE" list displays 2>/dev/null | jq -r '.[0].id' 2>/dev/null)
        if [[ -n "$display_id" && "$display_id" != "null" ]]; then
            run_gaze_with_exit capture --display "$display_id"
            if assert_exit 0 "CLI-E2E-017" "capture --display $display_id"; then
                pass "CLI-E2E-017" "capture --display $display_id"
            fi
        else
            skip "CLI-E2E-017" "no display id available"
        fi
    fi
fi

# ── CLI-E2E-018: capture --display 99999999 ──
if should_run "CLI-E2E-018"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-018" "capture bad display (no permission)"
    else
        run_gaze_with_exit capture --display 99999999
        if assert_exit 1 "CLI-E2E-018" "capture --display 99999999"; then
            pass "CLI-E2E-018" "capture --display 99999999 → exit 1"
        fi
    fi
fi

# ── CLI-E2E-020: capture --raw ──
if should_run "CLI-E2E-020"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-020" "capture --raw (no permission)"
    else
        run_gaze_with_exit capture --raw
        if assert_exit 0 "CLI-E2E-020" "capture --raw"; then
            local ow oh optw opth
            ow=$(echo "$LAST_STDOUT" | jq '.originalWidth' 2>/dev/null)
            oh=$(echo "$LAST_STDOUT" | jq '.originalHeight' 2>/dev/null)
            optw=$(echo "$LAST_STDOUT" | jq '.optimizedWidth' 2>/dev/null)
            opth=$(echo "$LAST_STDOUT" | jq '.optimizedHeight' 2>/dev/null)
            if [[ "$ow" == "$optw" && "$oh" == "$opth" ]]; then
                pass "CLI-E2E-020" "capture --raw keeps original dimensions"
            else
                fail "CLI-E2E-020" "capture --raw" "dimensions differ: ${ow}x${oh} vs ${optw}x${opth}"
            fi
        fi
    fi
fi

# ── CLI-E2E-021: capture --raw --format base64 (PNG magic bytes) ──
if should_run "CLI-E2E-021"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-021" "capture --raw --format base64 (no permission)"
    else
        run_gaze_with_exit capture --raw --format base64
        if assert_exit 0 "CLI-E2E-021" "raw base64"; then
            # PNG magic: \x89PNG → base64 starts with "iVBOR"
            if echo "$LAST_STDOUT" | grep -q "^iVBOR"; then
                pass "CLI-E2E-021" "raw base64 is PNG"
            else
                # Could also be valid if it's the raw format
                local first8
                first8=$(echo "$LAST_STDOUT" | head -c 8)
                fail "CLI-E2E-021" "raw base64" "not PNG magic (starts with: $first8)"
            fi
        fi
    fi
fi

# ── CLI-E2E-022: capture --window <valid_id> ──
if should_run "CLI-E2E-022"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-022" "capture --window (no permission)"
    else
        local window_id
        window_id=$("$GAZE" list windows 2>/dev/null | jq -r '[.[] | select(.isOnScreen == true)] | .[0].id // empty' 2>/dev/null)
        if [[ -n "$window_id" && "$window_id" != "null" ]]; then
            run_gaze_with_exit capture --window "$window_id"
            if assert_exit 0 "CLI-E2E-022" "capture --window $window_id"; then
                pass "CLI-E2E-022" "capture --window $window_id"
            fi
        else
            skip "CLI-E2E-022" "no window id available"
        fi
    fi
fi

# ── CLI-E2E-023: capture --window 99999999 ──
if should_run "CLI-E2E-023"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-023" "capture bad window (no permission)"
    else
        run_gaze_with_exit capture --window 99999999
        if assert_exit 1 "CLI-E2E-023" "capture --window 99999999"; then
            pass "CLI-E2E-023" "capture --window 99999999 → exit 1"
        fi
    fi
fi

# ── CLI-E2E-028: capture --provider gpt ──
if should_run "CLI-E2E-028"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-028" "capture --provider gpt (no permission)"
    else
        run_gaze_with_exit capture --provider gpt
        if assert_exit 0 "CLI-E2E-028" "capture --provider gpt" && \
           assert_json_value "provider" "Gpt4o" "CLI-E2E-028" "capture --provider gpt"; then
            pass "CLI-E2E-028" "capture --provider gpt → Gpt4o"
        fi
    fi
fi

# ── CLI-E2E-029: capture --provider gemini ──
if should_run "CLI-E2E-029"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-029" "capture --provider gemini (no permission)"
    else
        run_gaze_with_exit capture --provider gemini
        if assert_exit 0 "CLI-E2E-029" "capture --provider gemini" && \
           assert_json_value "provider" "Gemini" "CLI-E2E-029" "capture --provider gemini"; then
            pass "CLI-E2E-029" "capture --provider gemini → Gemini"
        fi
    fi
fi

# ── CLI-E2E-030: capture --output to unwritable path ──
if should_run "CLI-E2E-030"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-030" "capture unwritable path (no permission)"
    else
        run_gaze_with_exit capture --output /System/Library/gaze_nope.webp
        if assert_exit 1 "CLI-E2E-030" "unwritable output" && \
           assert_stderr_contains "Failed to write" "CLI-E2E-030" "unwritable output"; then
            pass "CLI-E2E-030" "unwritable path → exit 1"
        fi
    fi
fi

# ── CLI-E2E-031: robustness (5 sequential captures, no temp leak) ──
if should_run "CLI-E2E-031"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-031" "robustness (no permission)"
    else
        local all_ok=true
        local before_count after_count
        before_count=$(find /tmp -maxdepth 1 -name 'gaze_*.png' 2>/dev/null | wc -l | tr -d ' ')
        before_count=${before_count:-0}
        for i in 1 2 3 4 5; do
            run_gaze_with_exit capture --output "$TMP_DIR/robust_$i.webp" --format path
            if [[ "$LAST_EXIT" -ne 0 ]]; then
                all_ok=false
                break
            fi
        done
        after_count=$(find /tmp -maxdepth 1 -name 'gaze_*.png' 2>/dev/null | wc -l | tr -d ' ')
        after_count=${after_count:-0}
        if $all_ok; then
            if [[ "$after_count" -le "$before_count" ]]; then
                pass "CLI-E2E-031" "5 sequential captures, no temp leak"
            else
                fail "CLI-E2E-031" "robustness" "temp PNG leak: before=$before_count after=$after_count"
            fi
        else
            fail "CLI-E2E-031" "robustness" "capture $i failed"
        fi
    fi
fi

# ── CLI-E2E-044: --mode window --window <id> ──
if should_run "CLI-E2E-044"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-044" "mode window + window id (no permission)"
    else
        local window_id
        window_id=$("$GAZE" list windows 2>/dev/null | jq -r '[.[] | select(.isOnScreen == true)] | .[0].id // empty' 2>/dev/null)
        if [[ -n "$window_id" && "$window_id" != "null" ]]; then
            run_gaze_with_exit capture --mode window --window "$window_id"
            if assert_exit 0 "CLI-E2E-044" "--mode window --window $window_id"; then
                pass "CLI-E2E-044" "--mode window + --window id"
            fi
        else
            skip "CLI-E2E-044" "no window id available"
        fi
    fi
fi

# ── CLI-E2E-046: overwrite existing file ──
if should_run "CLI-E2E-046"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-046" "overwrite (no permission)"
    else
        local out_file="$TMP_DIR/overwrite_test.webp"
        echo "placeholder" > "$out_file"
        local before_hash
        before_hash=$(md5 -q "$out_file" 2>/dev/null || md5sum "$out_file" | cut -d' ' -f1)
        run_gaze_with_exit capture --output "$out_file" --format path
        if assert_exit 0 "CLI-E2E-046" "overwrite"; then
            local after_hash
            after_hash=$(md5 -q "$out_file" 2>/dev/null || md5sum "$out_file" | cut -d' ' -f1)
            if [[ "$before_hash" != "$after_hash" ]]; then
                pass "CLI-E2E-046" "existing file overwritten"
            else
                fail "CLI-E2E-046" "overwrite" "file content unchanged"
            fi
        fi
    fi
fi

# ── CLI-E2E-047: pipe integration (list → capture) ──
if should_run "CLI-E2E-047"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-047" "pipe integration (no permission)"
    else
        local window_id
        window_id=$("$GAZE" list windows 2>/dev/null | jq -r '[.[] | select(.isOnScreen == true)] | .[0].id // empty' 2>/dev/null)
        if [[ -n "$window_id" && "$window_id" != "null" ]]; then
            run_gaze_with_exit capture --window "$window_id" --format json
            if assert_exit 0 "CLI-E2E-047" "pipe integration"; then
                if echo "$LAST_STDOUT" | jq -e '.originalWidth' >/dev/null 2>&1; then
                    pass "CLI-E2E-047" "list → capture pipe integration"
                else
                    fail "CLI-E2E-047" "pipe integration" "invalid JSON output"
                fi
            fi
        else
            skip "CLI-E2E-047" "no window id available"
        fi
    fi
fi

echo ""
printf "${CYAN}── Optimize Tests ──${NC}\n"

# ── CLI-E2E-032: optimize --provider claude ──
if should_run "CLI-E2E-032"; then
    local out_file="$TMP_DIR/optimized_claude.webp"
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png" --provider claude --output "$out_file" --format json
    if assert_exit 0 "CLI-E2E-032" "optimize claude" && \
       assert_file_exists "$out_file" "CLI-E2E-032" "optimize claude" && \
       assert_json_key "outputPath" "CLI-E2E-032" "optimize claude" && \
       assert_json_value "provider" "Claude" "CLI-E2E-032" "optimize claude"; then
        # Check WebP magic bytes (RIFF....WEBP)
        local magic
        magic=$(xxd -l 4 "$out_file" | head -1)
        if echo "$magic" | grep -q "5249 4646"; then
            pass "CLI-E2E-032" "optimize --provider claude → WebP"
        else
            fail "CLI-E2E-032" "optimize claude" "not WebP magic bytes"
        fi
    fi
fi

# ── CLI-E2E-033: optimize --provider gpt → PNG ──
if should_run "CLI-E2E-033"; then
    local out_file="$TMP_DIR/optimized_gpt.png"
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png" --provider gpt --output "$out_file" --format path
    if assert_exit 0 "CLI-E2E-033" "optimize gpt"; then
        if assert_file_exists "$out_file" "CLI-E2E-033" "optimize gpt"; then
            local magic
            magic=$(xxd -l 4 "$out_file" | head -1)
            if echo "$magic" | grep -q "8950 4e47"; then
                pass "CLI-E2E-033" "optimize --provider gpt → PNG"
            else
                # GPT may also produce WebP - check
                if echo "$magic" | grep -q "5249 4646"; then
                    pass "CLI-E2E-033" "optimize --provider gpt → WebP (acceptable)"
                else
                    fail "CLI-E2E-033" "optimize gpt" "unknown format: $magic"
                fi
            fi
        fi
    fi
fi

# ── CLI-E2E-034: optimize --provider gemini → WebP ──
if should_run "CLI-E2E-034"; then
    local out_file="$TMP_DIR/optimized_gemini.webp"
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png" --provider gemini --output "$out_file" --format path
    if assert_exit 0 "CLI-E2E-034" "optimize gemini" && \
       assert_file_exists "$out_file" "CLI-E2E-034" "optimize gemini"; then
        local magic
        magic=$(xxd -l 4 "$out_file" | head -1)
        if echo "$magic" | grep -q "5249 4646"; then
            pass "CLI-E2E-034" "optimize --provider gemini → WebP"
        else
            fail "CLI-E2E-034" "optimize gemini" "not WebP: $magic"
        fi
    fi
fi

# ── CLI-E2E-035: optimize without output ──
if should_run "CLI-E2E-035"; then
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png"
    if assert_exit 0 "CLI-E2E-035" "optimize no output" && \
       assert_json_key "imageBase64" "CLI-E2E-035" "optimize no output"; then
        pass "CLI-E2E-035" "optimize without --output → JSON with base64"
    fi
fi

# ── CLI-E2E-036: optimize --format path requires --output ──
if should_run "CLI-E2E-036"; then
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png" --format path
    if assert_exit 2 "CLI-E2E-036" "optimize format path w/o output" && \
       assert_stderr_contains "format path requires --output" "CLI-E2E-036" "optimize format path w/o output"; then
        pass "CLI-E2E-036" "optimize --format path requires --output"
    fi
fi

# ── CLI-E2E-037: optimize non-existent file ──
if should_run "CLI-E2E-037"; then
    run_gaze_with_exit optimize /tmp/no_such_input.png
    if assert_exit 1 "CLI-E2E-037" "optimize missing file" && \
       assert_stderr_contains "Failed to read" "CLI-E2E-037" "optimize missing file"; then
        pass "CLI-E2E-037" "optimize missing file → exit 1"
    fi
fi

# ── CLI-E2E-038: optimize invalid image ──
if should_run "CLI-E2E-038"; then
    run_gaze_with_exit optimize "$FIXTURE_DIR/not_image.bin"
    if assert_exit 1 "CLI-E2E-038" "optimize invalid image"; then
        pass "CLI-E2E-038" "optimize invalid image → exit 1"
    fi
fi

# ── CLI-E2E-040: optimize large image (shrink check) ──
if should_run "CLI-E2E-040"; then
    run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_large_input.png" --provider claude
    if assert_exit 0 "CLI-E2E-040" "optimize large image"; then
        local ow optw
        ow=$(echo "$LAST_STDOUT" | jq '.originalWidth' 2>/dev/null)
        optw=$(echo "$LAST_STDOUT" | jq '.optimizedWidth' 2>/dev/null)
        if [[ "$optw" -lt "$ow" ]]; then
            pass "CLI-E2E-040" "large image shrunk (${ow} → ${optw})"
        else
            fail "CLI-E2E-040" "optimize large" "no shrink: original=$ow optimized=$optw"
        fi
    fi
fi

# ── CLI-E2E-041: JSON schema consistency (capture vs optimize) ──
if should_run "CLI-E2E-041"; then
    if ! $HAS_PERMISSION; then
        skip "CLI-E2E-041" "schema consistency (no permission)"
    else
        local capture_keys optimize_keys
        run_gaze_with_exit capture
        capture_keys=$(echo "$LAST_STDOUT" | jq -r 'keys[]' 2>/dev/null | sort)
        run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.png"
        optimize_keys=$(echo "$LAST_STDOUT" | jq -r 'keys[]' 2>/dev/null | sort)

        local required_keys="fileSize optimizedHeight optimizedWidth originalHeight originalWidth provider timestamp tokenEstimate"
        local all_present=true
        for key in $required_keys; do
            if ! echo "$capture_keys" | grep -q "^${key}$"; then
                fail "CLI-E2E-041" "schema consistency" "capture missing: $key"
                all_present=false
                break
            fi
            if ! echo "$optimize_keys" | grep -q "^${key}$"; then
                fail "CLI-E2E-041" "schema consistency" "optimize missing: $key"
                all_present=false
                break
            fi
        done
        if $all_present; then
            pass "CLI-E2E-041" "JSON schema consistent across commands"
        fi
    fi
fi

# ── CLI-E2E-045: optimize JPEG input ──
if should_run "CLI-E2E-045"; then
    if [[ -f "$FIXTURE_DIR/e2e_input.jpg" ]]; then
        local out_file="$TMP_DIR/optimized_from_jpeg.webp"
        run_gaze_with_exit optimize "$FIXTURE_DIR/e2e_input.jpg" --provider claude --output "$out_file" --format path
        if assert_exit 0 "CLI-E2E-045" "optimize JPEG" && \
           assert_file_exists "$out_file" "CLI-E2E-045" "optimize JPEG"; then
            pass "CLI-E2E-045" "optimize JPEG input → WebP"
        fi
    else
        skip "CLI-E2E-045" "JPEG fixture not available"
    fi
fi

echo ""
printf "${CYAN}── Permission Tests (auto-checkable) ──${NC}\n"

# ── CLI-E2E-009/010: These are marked manual but we can partially verify ──
# If we have permission, we can't test the denial path automatically
if should_run "CLI-E2E-009"; then
    skip "CLI-E2E-009" "permission denied test (manual: requires removing permission)"
fi
if should_run "CLI-E2E-010"; then
    skip "CLI-E2E-010" "list permission denied test (manual: requires removing permission)"
fi

echo ""
printf "${CYAN}── Manual Tests (skipped in auto mode) ──${NC}\n"

for id in CLI-E2E-019 CLI-E2E-024 CLI-E2E-025 CLI-E2E-026 CLI-E2E-027 CLI-E2E-039 CLI-E2E-042 CLI-E2E-048; do
    if should_run "$id"; then
        case "$id" in
            CLI-E2E-019) skip "$id" "capture --copy clipboard verification (manual)" ;;
            CLI-E2E-024) skip "$id" "interactive window capture (manual)" ;;
            CLI-E2E-025) skip "$id" "window capture cancel (manual)" ;;
            CLI-E2E-026) skip "$id" "interactive area capture (manual)" ;;
            CLI-E2E-027) skip "$id" "area capture cancel (manual)" ;;
            CLI-E2E-039) skip "$id" "optimize --copy clipboard verification (manual)" ;;
            CLI-E2E-042) skip "$id" "multi-display comparison (manual)" ;;
            CLI-E2E-048) skip "$id" "Japanese encoding (manual)" ;;
        esac
    fi
done

# ── Summary ──────────────────────────────────────────────────

echo ""
printf "${CYAN}═══ Summary ═══${NC}\n"
printf "Total: %d | ${GREEN}Pass: %d${NC} | ${RED}Fail: %d${NC} | ${YELLOW}Skip: %d${NC}\n" "$TOTAL" "$PASS" "$FAIL" "$SKIP"

if [[ "$FAIL" -gt 0 ]]; then
    echo ""
    printf "${RED}Failed tests:${NC}\n"
    for r in "${RESULTS[@]}"; do
        if echo -e "$r" | grep -q "FAIL"; then
            echo -e "  $r"
        fi
    done
    return 1
fi

return 0
}

main
exit $?
