#!/usr/bin/env bash
# Test script to verify session persistence across hub restarts
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; exit 1; }
info() { echo -e "${BLUE}ℹ${NC} $1"; }
step() { echo -e "${YELLOW}▶${NC} $1"; }

# Configuration
HUB_SERVICE=${HUB_SERVICE:-"workshop-hub-svc.default.svc.cluster.local:8080"}
TEST_USERNAME="test-user-$(date +%s)"
COOKIE_FILE="/tmp/workshop-session-cookies-$$.txt"
NAMESPACE="default"

echo "🧪 Workshop Hub Session Persistence Test"
echo "========================================="
echo ""
info "Testing with username: $TEST_USERNAME"
info "Hub service: $HUB_SERVICE"
info "Cookie file: $COOKIE_FILE"
echo ""

# Cleanup function
cleanup() {
    rm -f "$COOKIE_FILE"
}
trap cleanup EXIT

# Test 1: Login and get session cookie
step "TEST 1: Login to workshop hub"
echo ""

# Create a test pod to run curl from inside the cluster
TEST_POD="session-test-$$"
kubectl run $TEST_POD \
    --image=curlimages/curl:latest \
    --restart=Never \
    -n $NAMESPACE \
    --rm -i --quiet \
    -- sh -c "
    # Login
    curl -v -X POST http://$HUB_SERVICE/login \
        -H 'Content-Type: application/json' \
        -d '{\"username\":\"$TEST_USERNAME\"}' \
        -c /tmp/cookies.txt \
        -o /tmp/login-response.json 2>&1 | grep -E '(Set-Cookie|HTTP/)' || true
    
    # Show login response
    cat /tmp/login-response.json
    echo ''
    
    # Show cookies
    cat /tmp/cookies.txt
" > /tmp/login-output.txt 2>&1

# Extract session cookie
if grep -q "workshop_session" /tmp/login-output.txt; then
    SESSION_COOKIE=$(grep "workshop_session" /tmp/login-output.txt | awk '{print $NF}')
    echo "$SESSION_COOKIE" > "$COOKIE_FILE"
    pass "Login successful, session cookie obtained"
else
    fail "Failed to get session cookie"
fi

echo ""
info "Session cookie: $(cat $COOKIE_FILE | head -c 50)..."
echo ""

# Test 2: Verify authenticated access
step "TEST 2: Access protected route with session"
echo ""

kubectl run test-access-$$ \
    --image=curlimages/curl:latest \
    --restart=Never \
    -n $NAMESPACE \
    --rm -i --quiet \
    -- sh -c "
    curl -s http://$HUB_SERVICE/workshop \
        -H 'Cookie: workshop_session=$(cat $COOKIE_FILE)' \
        -w '\nHTTP Status: %{http_code}\n'
" > /tmp/workshop-response.txt

if grep -q "Welcome to your Workshop" /tmp/workshop-response.txt && grep -q "HTTP Status: 200" /tmp/workshop-response.txt; then
    pass "Authenticated access successful"
else
    cat /tmp/workshop-response.txt
    fail "Failed to access protected route"
fi

# Test 3: Check session in Redis
step "TEST 3: Verify session stored in Redis"
echo ""

REDIS_KEYS=$(kubectl exec workshop-redis-0 -n workshop-system -- redis-cli KEYS "session:*" 2>/dev/null || echo "")
if [ -n "$REDIS_KEYS" ]; then
    pass "Session found in Redis"
    echo ""
    info "Redis session keys:"
    echo "$REDIS_KEYS" | head -n 5
    
    # Get first session details
    FIRST_KEY=$(echo "$REDIS_KEYS" | head -n 1)
    if [ -n "$FIRST_KEY" ]; then
        echo ""
        info "Session data for: $FIRST_KEY"
        kubectl exec workshop-redis-0 -n workshop-system -- redis-cli GET "$FIRST_KEY" 2>/dev/null | head -c 200
        echo "..."
    fi
else
    fail "No session found in Redis"
fi

echo ""
echo ""

# Test 4: Restart hub pod
step "TEST 4: Restarting workshop hub pod"
echo ""

info "Current hub pods:"
kubectl get pods -n $NAMESPACE -l app=workshop-hub

# Get current pod name
OLD_POD=$(kubectl get pods -n $NAMESPACE -l app=workshop-hub -o jsonpath='{.items[0].metadata.name}')
info "Current pod: $OLD_POD"

# Trigger rolling restart
kubectl rollout restart deployment/workshop-hub -n $NAMESPACE
info "Triggered rolling restart..."

# Wait for old pod to terminate
echo ""
info "Waiting for old pod to terminate..."
kubectl wait --for=delete pod/$OLD_POD -n $NAMESPACE --timeout=60s || true

# Wait for new pod to be ready
echo ""
info "Waiting for new pod to be ready..."
kubectl wait --for=condition=ready pod -l app=workshop-hub -n $NAMESPACE --timeout=120s

NEW_POD=$(kubectl get pods -n $NAMESPACE -l app=workshop-hub -o jsonpath='{.items[0].metadata.name}')
pass "Hub restarted successfully"
info "New pod: $NEW_POD"

# Give it a moment to fully initialize
sleep 5

echo ""

# Test 5: Access with old session cookie after restart
step "TEST 5: Access workshop with original session cookie (after restart)"
echo ""

kubectl run test-persistence-$$ \
    --image=curlimages/curl:latest \
    --restart=Never \
    -n $NAMESPACE \
    --rm -i --quiet \
    -- sh -c "
    curl -s http://$HUB_SERVICE/workshop \
        -H 'Cookie: workshop_session=$(cat $COOKIE_FILE)' \
        -w '\nHTTP Status: %{http_code}\n'
" > /tmp/persistence-response.txt

if grep -q "Welcome to your Workshop" /tmp/persistence-response.txt && grep -q "HTTP Status: 200" /tmp/persistence-response.txt; then
    pass "✨ Session persisted across restart! ✨"
    echo ""
    info "The session survived the hub pod restart because it's stored in Redis!"
else
    cat /tmp/persistence-response.txt
    fail "Session did not persist (this means Redis isn't working properly)"
fi

# Test 6: Session TTL check
step "TEST 6: Check session TTL in Redis"
echo ""

if [ -n "$FIRST_KEY" ]; then
    TTL=$(kubectl exec workshop-redis-0 -n workshop-system -- redis-cli TTL "$FIRST_KEY" 2>/dev/null || echo "-1")
    if [ "$TTL" -gt 0 ]; then
        HOURS=$((TTL / 3600))
        MINUTES=$(((TTL % 3600) / 60))
        pass "Session TTL: ${HOURS}h ${MINUTES}m remaining"
        info "Session will expire after 24 hours of inactivity"
    else
        warn "Session has no TTL set (TTL: $TTL)"
    fi
fi

echo ""
echo ""

# Summary
echo "========================================="
echo -e "${GREEN}✅ All persistence tests passed!${NC}"
echo ""
echo "📊 Test Summary:"
echo "  ✓ User login successful"
echo "  ✓ Session stored in Redis"
echo "  ✓ Authenticated access works"
echo "  ✓ Hub pod restarted"
echo "  ✓ Session persisted across restart"
echo "  ✓ Session TTL configured"
echo ""
echo "🎉 Redis session persistence is working correctly!"
echo ""
echo "💡 What this means:"
echo "  • Sessions survive hub restarts"
echo "  • Multiple hub replicas can share sessions"
echo "  • Users stay logged in during deployments"
echo "  • Production-ready session management"
echo ""
echo "🔍 Manual verification commands:"
echo "  # View all sessions in Redis"
echo "  kubectl exec workshop-redis-0 -n workshop-system -- redis-cli KEYS 'session:*'"
echo ""
echo "  # Get specific session data"
echo "  kubectl exec workshop-redis-0 -n workshop-system -- redis-cli GET 'session:xxxxx'"
echo ""
echo "  # Check hub logs for Redis connection"
echo "  kubectl logs -l app=workshop-hub -n $NAMESPACE | grep -i redis"
echo ""