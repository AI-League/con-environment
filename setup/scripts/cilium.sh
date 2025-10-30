#!/usr/bin/env bash
set -euo pipefail

CILIUM_VERSION="1.16.5"
VALUES_FILE="setup/k8/cilium-values.yaml"
OUTPUT_FILE="talos/cilium.yaml"

echo "Generating Cilium manifests..."
CILIUM_CHART=$(helm template cilium cilium/cilium \
  --version "$CILIUM_VERSION" \
  --namespace kube-system \
  --values "$VALUES_FILE")

echo "Creating Talos patch..."
cat > "$OUTPUT_FILE" <<'EOF'
# This is checked in and has keys. 
# This is for development only and keys don't touch anything we care about. 
# We'll automate this in a bit.
cluster:
  network:
    cni:
      name: none
  proxy:
    disabled: true
  inlineManifests:
    - name: cilium
      contents: |
        ---
EOF

echo "$CILIUM_CHART" | sed 's/^/        /' >> "$OUTPUT_FILE"

echo "✓ Patch written to $OUTPUT_FILE"