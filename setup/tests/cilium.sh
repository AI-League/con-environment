#!/usr/bin/env bash
set -e

echo "=== Cilium Pods ==="
kubectl get pods -n kube-system -l app.kubernetes.io/name=cilium-agent

echo -e "\n=== kube-proxy (should be empty) ==="
kubectl get ds -n kube-system kube-proxy 2>&1 || echo "✓ kube-proxy not found (expected)"

echo -e "\n=== Cilium Config ==="
kubectl get cm -n kube-system cilium-config -o yaml | grep -E "kube-proxy-replacement|ipam|k8s-service"

echo -e "\n=== Cilium Endpoints ==="
kubectl get ciliumendpoints -A | head -n 5