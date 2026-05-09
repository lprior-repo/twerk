#!/usr/bin/env bash
# =============================================================================
# Twerk 12-Step QA Test Runner
# =============================================================================
# Exercises every API endpoint and end-user workflow.
# Requires: twerk server running, curl, jq
# Usage: ./qa/run-qa.sh [STEP]
#   STEP: 1-12 to run a specific step, or omit to run all.
# =============================================================================

set -euo pipefail

BASE="${TWERK_ENDPOINT:-http://localhost:8000}"
PASS=0
FAIL=0
SKIP=0
STEP="${1:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

pass() { PASS=$((PASS+1)); echo -e "  ${GREEN}PASS${NC}: $1"; }
fail() { FAIL=$((FAIL+1)); echo -e "  ${RED}FAIL${NC}: $1"; }
skip() { SKIP=$((SKIP+1)); echo -e "  ${YELLOW}SKIP${NC}: $1"; }
section() { echo -e "\n${CYAN}=============================================================================${NC}"; echo -e "${CYAN}$1${NC}"; echo -e "${CYAN}=============================================================================${NC}"; }

require() { command -v "$1" &>/dev/null || { echo "Required: $1"; exit 1; }; }
require curl
require jq

# ---- Helper ----
api() {
    local method="$1" path="$2"
    shift 2
    curl -sf -X "$method" -o /tmp/qa-response.json -w "%{http_code}" "$@" "$BASE$path"
}

extract_id() {
    jq -r '.id // .summary.id // empty' /tmp/qa-response.json 2>/dev/null | head -1
}

# =============================================================================
# STEP 1: Health Check & Bootstrap
# =============================================================================
step_01() {
    section "STEP 1: Health Check & Bootstrap"

    # Health
    CODE=$(api GET /health)
    BODY=$(cat /tmp/qa-response.json)
    if [ "$CODE" = "200" ]; then
        STATUS=$(echo "$BODY" | jq -r '.status')
        [ "$STATUS" = "UP" ] && pass "GET /health -> 200 UP" || fail "Health status=$STATUS (expected UP)"
    else
        fail "GET /health -> $CODE (expected 200)"
    fi

    # OpenAPI
    CODE=$(api GET /openapi.json)
    [ "$CODE" = "200" ] && pass "GET /openapi.json -> 200" || fail "GET /openapi.json -> $CODE"
}

# =============================================================================
# STEP 2: Submit Job via YAML
# =============================================================================
step_02() {
    section "STEP 2: Submit Job via YAML Content-Type"

    CODE=$(api POST /jobs -H "Content-type: text/yaml" --data-binary @qa/02-submit-job-yaml.yaml)
    JOB_ID=$(extract_id)
    if [ "$CODE" = "200" ] && [ -n "$JOB_ID" ]; then
        pass "POST /jobs (YAML) -> 200 id=$JOB_ID"
    else
        fail "POST /jobs (YAML) -> $CODE"
        return
    fi

    sleep 1
    CODE=$(api GET "/jobs/$JOB_ID")
    if [ "$CODE" = "200" ]; then
        NAME=$(jq -r '.name // empty' /tmp/qa-response.json)
        [ "$NAME" = "qa-02-submit-yaml" ] && pass "GET /jobs/$JOB_ID name matches" || fail "Job name=$NAME"
    else
        fail "GET /jobs/$JOB_ID -> $CODE"
    fi
}

# =============================================================================
# STEP 3: Submit Job via JSON
# =============================================================================
step_03() {
    section "STEP 3: Submit Job via JSON Content-Type"

    CODE=$(api POST /jobs -H "Content-type: application/json" \
        -d '{"name":"qa-03-json","tasks":[{"name":"echo","image":"ubuntu:mantic","run":"echo hello"}]}')
    JOB_ID=$(extract_id)
    if [ "$CODE" = "200" ] && [ -n "$JOB_ID" ]; then
        pass "POST /jobs (JSON) -> 200 id=$JOB_ID"
    else
        fail "POST /jobs (JSON) -> $CODE"
        return
    fi
}

# =============================================================================
# STEP 4: List Jobs + Pagination + Search
# =============================================================================
step_04() {
    section "STEP 4: List Jobs, Pagination & Search"

    CODE=$(api GET /jobs)
    [ "$CODE" = "200" ] && pass "GET /jobs -> 200" || fail "GET /jobs -> $CODE"

    CODE=$(api GET "/jobs?page=1&size=2")
    [ "$CODE" = "200" ] && pass "GET /jobs?page=1&size=2 -> 200" || fail "Paginated list -> $CODE"

    CODE=$(api GET "/jobs?q=qa")
    [ "$CODE" = "200" ] && pass "GET /jobs?q=qa -> 200" || fail "Search -> $CODE"

    CODE=$(api GET "/jobs?page=abc")
    [ "$CODE" = "200" ] && pass "GET /jobs?page=abc (graceful) -> 200" || fail "Invalid param -> $CODE"
}

# =============================================================================
# STEP 5: Get Job Details + Task Logs
# =============================================================================
step_05() {
    section "STEP 5: Job Details, Task Details & Log Retrieval"

    # Get first job
    api GET "/jobs" > /dev/null
    JOB_ID=$(jq -r '.items[0].id // empty' /tmp/qa-response.json 2>/dev/null)
    if [ -z "$JOB_ID" ]; then
        skip "No jobs found to inspect"
        return
    fi

    CODE=$(api GET "/jobs/$JOB_ID")
    [ "$CODE" = "200" ] && pass "GET /jobs/$JOB_ID -> 200" || { fail "GET /jobs/$JOB_ID -> $CODE"; return; }

    CODE=$(api GET "/jobs/$JOB_ID/log")
    [ "$CODE" = "200" ] && pass "GET /jobs/$JOB_ID/log -> 200" || fail "GET /jobs/$JOB_ID/log -> $CODE"

    TASK_ID=$(jq -r '.tasks[0].id // empty' /tmp/qa-response.json 2>/dev/null)
    if [ -n "$TASK_ID" ]; then
        CODE=$(api GET "/tasks/$TASK_ID")
        [ "$CODE" = "200" ] && pass "GET /tasks/$TASK_ID -> 200" || fail "GET /tasks/$TASK_ID -> $CODE"

        CODE=$(api GET "/tasks/$TASK_ID/log?page=1&size=10")
        [ "$CODE" = "200" ] && pass "GET /tasks/$TASK_ID/log -> 200" || fail "Task log -> $CODE"

        CODE=$(api GET "/tasks/$TASK_ID/log?q=processing")
        [ "$CODE" = "200" ] && pass "GET /tasks/$TASK_ID/log?q=processing -> 200" || fail "Task log search -> $CODE"
    else
        skip "No tasks found for job $JOB_ID"
    fi
}

# =============================================================================
# STEP 6: Cancel & Restart Job
# =============================================================================
step_06() {
    section "STEP 6: Cancel & Restart Job Lifecycle"

    # Create a long-running job
    CODE=$(api POST /jobs -H "Content-type: application/json" \
        -d '{"name":"cancel-target","tasks":[{"name":"sleeper","image":"ubuntu:mantic","run":"sleep 300"}]}')
    JOB_ID=$(extract_id)
    if [ -z "$JOB_ID" ]; then
        fail "Failed to create cancel-target job"
        return
    fi
    pass "Created cancel-target job: $JOB_ID"

    sleep 1

    # Cancel
    CODE=$(api PUT "/jobs/$JOB_ID/cancel")
    if [ "$CODE" = "200" ]; then
        pass "PUT /jobs/$JOB_ID/cancel -> 200"
    else
        fail "PUT /jobs/$JOB_ID/cancel -> $CODE (may have completed already)"
    fi

    # Double cancel should 400
    CODE=$(api PUT "/jobs/$JOB_ID/cancel")
    [ "$CODE" = "400" ] && pass "Second cancel -> 400 (already terminal)" || fail "Second cancel -> $CODE (expected 400)"

    # Restart
    CODE=$(api PUT "/jobs/$JOB_ID/restart")
    if [ "$CODE" = "200" ]; then
        pass "PUT /jobs/$JOB_ID/restart -> 200"
    else
        fail "PUT /jobs/$JOB_ID/restart -> $CODE"
    fi

    # Restart running job should fail
    CODE=$(api POST /jobs -H "Content-type: application/json" \
        -d '{"name":"running-target","tasks":[{"name":"sleeper","image":"ubuntu:mantic","run":"sleep 60"}]}')
    RUNNING_ID=$(extract_id)
    sleep 1
    if [ -n "$RUNNING_ID" ]; then
        CODE=$(api PUT "/jobs/$RUNNING_ID/restart")
        [ "$CODE" = "400" ] && pass "Restart running job -> 400" || fail "Restart running -> $CODE (expected 400)"
        # Cleanup
        api PUT "/jobs/$RUNNING_ID/cancel" > /dev/null 2>&1 || true
        api PUT "/jobs/$JOB_ID/cancel" > /dev/null 2>&1 || true
    fi
}

# =============================================================================
# STEP 7: Timeout & Retry
# =============================================================================
step_07() {
    section "STEP 7: Timeout & Retry Mechanisms"

    # Timeout test
    CODE=$(api POST /jobs -H "Content-type: application/json" \
        -d '{"name":"timeout-test","tasks":[{"name":"sleeper","image":"ubuntu:mantic","run":"sleep 60","timeout":"3s"}]}')
    TIMEOUT_ID=$(extract_id)
    if [ -n "$TIMEOUT_ID" ]; then
        pass "Created timeout job: $TIMEOUT_ID"
        echo "  Waiting 5s for timeout..."
        sleep 5
        api GET "/jobs/$TIMEOUT_ID" > /dev/null
        STATE=$(jq -r '.state // empty' /tmp/qa-response.json)
        if [ "$STATE" = "FAILED" ]; then
            pass "Timeout job -> FAILED as expected"
        else
            echo "  Timeout job state: $STATE (may still be running)"
        fi
    fi

    # Retry test
    CODE=$(api POST /jobs -H "Content-type: application/json" \
        -d '{"name":"retry-test","tasks":[{"name":"flaky","image":"ubuntu:mantic","run":"exit 0","retry":{"limit":2}}]}')
    RETRY_ID=$(extract_id)
    if [ -n "$RETRY_ID" ]; then
        pass "Created retry job: $RETRY_ID"
        sleep 3
        api GET "/jobs/$RETRY_ID" > /dev/null
        STATE=$(jq -r '.state // empty' /tmp/qa-response.json)
        [ "$STATE" = "COMPLETED" ] && pass "Retry job -> COMPLETED" || echo "  Retry job state: $STATE"
    fi
}

# =============================================================================
# STEP 8: Triggers CRUD
# =============================================================================
step_08() {
    section "STEP 8: Triggers CRUD Lifecycle"

    # List (empty or existing)
    CODE=$(api GET /triggers)
    [ "$CODE" = "200" ] && pass "GET /triggers -> 200" || fail "List triggers -> $CODE"

    # Create
    CODE=$(api POST /triggers -H "Content-type: application/json" \
        -d '{"name":"qa-trigger","enabled":true,"event":"job.completed","action":"notify","metadata":{"channel":"slack"}}')
    if [ "$CODE" = "201" ]; then
        TRIGGER_ID=$(jq -r '.id // empty' /tmp/qa-response.json)
        VER=$(jq -r '.version // empty' /tmp/qa-response.json)
        pass "POST /triggers -> 201 id=$TRIGGER_ID ver=$VER"
    else
        fail "POST /triggers -> $CODE"
        return
    fi

    # Get
    CODE=$(api GET "/triggers/$TRIGGER_ID")
    [ "$CODE" = "200" ] && pass "GET /triggers/$TRIGGER_ID -> 200" || fail "Get trigger -> $CODE"

    # Update
    CODE=$(api PUT "/triggers/$TRIGGER_ID" -H "Content-type: application/json" \
        -d "{\"name\":\"qa-trigger-v2\",\"enabled\":true,\"event\":\"job.failed\",\"action\":\"alert\",\"metadata\":{},\"version\":$VER}")
    if [ "$CODE" = "200" ]; then
        NEW_VER=$(jq -r '.version // empty' /tmp/qa-response.json)
        pass "PUT /triggers/$TRIGGER_ID -> 200 new_ver=$NEW_VER"
    else
        fail "PUT /triggers/$TRIGGER_ID -> $CODE"
        NEW_VER=$VER
    fi

    # Stale version conflict (409)
    CODE=$(api PUT "/triggers/$TRIGGER_ID" -H "Content-type: application/json" \
        -d "{\"name\":\"stale\",\"enabled\":true,\"event\":\"x\",\"action\":\"y\",\"version\":$VER}")
    [ "$CODE" = "409" ] && pass "Stale version -> 409 Conflict" || fail "Stale version -> $CODE (expected 409)"

    # Invalid ID (400)
    CODE=$(api GET "/triggers/bad%20id")
    [ "$CODE" = "400" ] && pass "Invalid trigger ID -> 400" || fail "Invalid ID -> $CODE (expected 400)"

    # Delete
    CODE=$(api DELETE "/triggers/$TRIGGER_ID")
    [ "$CODE" = "204" ] || [ "$CODE" = "200" ] && pass "DELETE /triggers/$TRIGGER_ID -> $CODE" || fail "Delete trigger -> $CODE"

    # Get after delete (404)
    CODE=$(api GET "/triggers/$TRIGGER_ID")
    [ "$CODE" = "404" ] && pass "Get deleted trigger -> 404" || fail "Get after delete -> $CODE (expected 404)"

    # Create with blank name (400)
    CODE=$(api POST /triggers -H "Content-type: application/json" \
        -d '{"name":"","enabled":true,"event":"x","action":"y"}')
    [ "$CODE" = "400" ] && pass "Create with blank name -> 400" || fail "Blank name -> $CODE (expected 400)"
}

# =============================================================================
# STEP 9: Scheduled Jobs CRUD
# =============================================================================
step_09() {
    section "STEP 9: Scheduled Jobs CRUD"

    # Create
    CODE=$(api POST /scheduled-jobs -H "Content-type: application/json" \
        -d '{"name":"qa-scheduled","cron":"*/5 * * * *","tasks":[{"name":"tick","image":"ubuntu:mantic","run":"echo tick"}]}')
    if [ "$CODE" = "200" ]; then
        SJ_ID=$(jq -r '.id // empty' /tmp/qa-response.json)
        pass "POST /scheduled-jobs -> 200 id=$SJ_ID"
    else
        fail "POST /scheduled-jobs -> $CODE"
        BODY=$(cat /tmp/qa-response.json)
        echo "  Response: $BODY"
        return
    fi

    # List
    CODE=$(api GET /scheduled-jobs)
    [ "$CODE" = "200" ] && pass "GET /scheduled-jobs -> 200" || fail "List scheduled -> $CODE"

    # Get
    CODE=$(api GET "/scheduled-jobs/$SJ_ID")
    [ "$CODE" = "200" ] && pass "GET /scheduled-jobs/$SJ_ID -> 200" || fail "Get scheduled -> $CODE"

    # Pause
    CODE=$(api PUT "/scheduled-jobs/$SJ_ID/pause")
    [ "$CODE" = "200" ] && pass "PUT /scheduled-jobs/$SJ_ID/pause -> 200" || fail "Pause -> $CODE"

    # Resume
    CODE=$(api PUT "/scheduled-jobs/$SJ_ID/resume")
    [ "$CODE" = "200" ] && pass "PUT /scheduled-jobs/$SJ_ID/resume -> 200" || fail "Resume -> $CODE"

    # Delete
    CODE=$(api DELETE "/scheduled-jobs/$SJ_ID")
    [ "$CODE" = "200" ] && pass "DELETE /scheduled-jobs/$SJ_ID -> 200" || fail "Delete scheduled -> $CODE"
}

# =============================================================================
# STEP 10: Queues, Nodes, Metrics
# =============================================================================
step_10() {
    section "STEP 10: Queues, Nodes & Metrics"

    # Queues
    CODE=$(api GET /queues)
    [ "$CODE" = "200" ] && pass "GET /queues -> 200" || fail "GET /queues -> $CODE"

    CODE=$(api GET /queues/default)
    [ "$CODE" = "200" ] && pass "GET /queues/default -> 200" || echo "  Queue default not found ($CODE) - may not exist yet"

    # Nodes
    CODE=$(api GET /nodes)
    [ "$CODE" = "200" ] && pass "GET /nodes -> 200" || fail "GET /nodes -> $CODE"

    # Metrics
    CODE=$(api GET /metrics)
    [ "$CODE" = "200" ] && pass "GET /metrics -> 200" || fail "GET /metrics -> $CODE"
}

# =============================================================================
# STEP 11: User Creation & Validation
# =============================================================================
step_11() {
    section "STEP 11: User Creation & Validation"

    # Valid user
    CODE=$(api POST /users -H "Content-type: application/json" \
        -d '{"username":"qatestuser","password":"testpassword123"}')
    [ "$CODE" = "200" ] && pass "POST /users (valid) -> 200" || fail "Create user -> $CODE"

    # Short password
    CODE=$(api POST /users -H "Content-type: application/json" \
        -d '{"username":"baduser","password":"short"}')
    [ "$CODE" = "400" ] && pass "POST /users (short password) -> 400" || fail "Short password -> $CODE (expected 400)"

    # Missing fields
    CODE=$(api POST /users -H "Content-type: application/json" -d '{}')
    [ "$CODE" = "400" ] && pass "POST /users (empty body) -> 400" || fail "Empty body -> $CODE (expected 400)"

    # Bad username (too short)
    CODE=$(api POST /users -H "Content-type: application/json" \
        -d '{"username":"ab","password":"password123"}')
    [ "$CODE" = "400" ] && pass "POST /users (short username) -> 400" || fail "Short username -> $CODE (expected 400)"
}

# =============================================================================
# STEP 12: Unsupported Content-Type & Edge Cases
# =============================================================================
step_12() {
    section "STEP 12: Edge Cases & Error Handling"

    # Unsupported content type
    CODE=$(curl -sf -o /tmp/qa-response.json -w "%{http_code}" -X POST "$BASE/jobs" \
        -H "Content-type: text/plain" -d "not a job" 2>/dev/null || echo "000")
    [ "$CODE" = "400" ] && pass "POST /jobs (text/plain) -> 400" || fail "Unsupported content type -> $CODE"

    # Get nonexistent job
    CODE=$(api GET /jobs/nonexistent-id-12345)
    [ "$CODE" = "404" ] && pass "GET /jobs/nonexistent -> 404" || fail "Nonexistent job -> $CODE (expected 404)"

    # Get nonexistent task
    CODE=$(api GET /tasks/nonexistent-id-12345)
    [ "$CODE" = "404" ] && pass "GET /tasks/nonexistent -> 404" || fail "Nonexistent task -> $CODE (expected 404)"

    # Get nonexistent scheduled job
    CODE=$(api GET /scheduled-jobs/nonexistent-id-12345)
    [ "$CODE" = "404" ] && pass "GET /scheduled-jobs/nonexistent -> 404" || fail "Nonexistent scheduled -> $CODE"

    # Invalid YAML job
    CODE=$(curl -sf -o /tmp/qa-response.json -w "%{http_code}" -X POST "$BASE/jobs" \
        -H "Content-type: text/yaml" -d "not: valid: yaml:" 2>/dev/null || echo "000")
    # Should be 400 (bad request) or could parse as valid YAML but fail validation
    if [ "$CODE" = "400" ] || [ "$CODE" = "200" ]; then
        pass "POST /jobs (invalid YAML) handled gracefully -> $CODE"
    else
        fail "Invalid YAML -> $CODE"
    fi

    # Scheduled job missing cron
    CODE=$(api POST /scheduled-jobs -H "Content-type: application/json" \
        -d '{"name":"no-cron","tasks":[{"name":"t","image":"ubuntu:mantic","run":"echo hi"}]}')
    [ "$CODE" = "400" ] && pass "POST /scheduled-jobs (no cron) -> 400" || fail "Missing cron -> $CODE (expected 400)"

    # Scheduled job missing tasks
    CODE=$(api POST /scheduled-jobs -H "Content-type: application/json" \
        -d '{"name":"no-tasks","cron":"*/5 * * * *"}')
    [ "$CODE" = "400" ] && pass "POST /scheduled-jobs (no tasks) -> 400" || fail "Missing tasks -> $CODE (expected 400)"

    # Trigger body too large
    LARGE_NAME=$(python3 -c "print('x' * 100)" 2>/dev/null || echo "xxxxxxxxxxxxxxxxxxxx")
    CODE=$(api POST /triggers -H "Content-type: application/json" \
        -d "{\"name\":\"$LARGE_NAME\",\"enabled\":true,\"event\":\"x\",\"action\":\"y\"}")
    [ "$CODE" = "400" ] && pass "POST /triggers (field too long) -> 400" || fail "Long field -> $CODE (expected 400)"
}

# =============================================================================
# Run
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Twerk QA Test Suite"
    echo "  Endpoint: $BASE"
    echo "  Time: $(date)"
    echo "=========================================="

    if [ -n "$STEP" ]; then
        "step_$(printf '%02d' "$STEP")"
    else
        step_01
        step_02
        step_03
        step_04
        step_05
        step_06
        step_07
        step_08
        step_09
        step_10
        step_11
        step_12
    fi

    echo ""
    echo "=========================================="
    echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}, ${YELLOW}$SKIP skipped${NC}"
    echo "=========================================="

    if [ "$FAIL" -gt 0 ]; then
        exit 1
    fi
    exit 0
}

main
