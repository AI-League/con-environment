#!/usr/bin/env bash
set -euo pipefail

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}✓${NC} $1"
}

log_step() {
    echo -e "${YELLOW}➜${NC} $1"
}

echo "⚙️  Starting Rook Ceph installation..."
echo ""

# Step 1: Add Helm repository
log_step "1/6: Adding Rook Ceph Helm repository..."
helm repo add rook-release https://charts.rook.io/release
helm repo update
log_info "Helm repository added"
echo ""


# Step 2: Install operator
log_step "2/6: Installing Rook Ceph operator..."
helm install --create-namespace --namespace rook-ceph rook-ceph rook-release/rook-ceph --wait
log_info "Operator installed"
echo ""

# Step 3: Label namespace
log_step "3/6: Labeling namespace for pod security..."
kubectl label namespace rook-ceph pod-security.kubernetes.io/enforce=privileged
log_info "Namespace labeled"
echo ""

# Step 4: Install cluster
log_step "4/6: Installing Rook Ceph cluster..."
helm install --create-namespace --namespace rook-ceph rook-ceph-cluster \
    --set operatorNamespace=rook-ceph rook-release/rook-ceph-cluster --wait
log_info "Cluster installed"
echo ""

# Step 5: Apply custom configurations from k8/rook_ceph
log_step "5/6: Applying Rook Ceph configurations..."
if [ -d "k8/rook_ceph" ]; then
    kubectl apply -f k8/rook_ceph/
    log_info "Configurations applied from k8/rook_ceph/"
else
    log_info "No additional configurations found in k8/rook_ceph/"
fi
echo ""

echo "=========================================="
echo "✅ Rook Ceph installation complete!"
echo "=========================================="
echo ""
log_info "Next steps:"
echo "  1. Run tests: ./setup/tests/test-rook-ceph.sh"
echo "  2. Check cluster status:"
echo "     kubectl -n rook-ceph exec -it deploy/rook-ceph-tools -- ceph status"
echo ""