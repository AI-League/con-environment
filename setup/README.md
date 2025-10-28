# Setup

Here's all the charts, scripts and patches we need to get the k8 cluster to a usable state with:
- Rook + Ceph
- Cilium
- ... whatever else we need.

This is meant to take a fresh cluster and configure it.

If you want to update the cilium chart, change the values in `k8/cilium-values.yaml`, then run `scripts/cilium.sh`. This sets up the inline manifest patch.