# con_shell - Physical Cluster Configuration Tool

Simple Nix-based tool for deploying Talos Linux to physical hardware.

## Philosophy

**con_shell generates configurations. You apply them manually.**

- ✅ Generate machine configs with proper patches
- ✅ Discover disks on nodes
- ✅ Reset nodes before/after conference

## Quick Start

```bash
# 1. Enter environment
nix develop .#physical

# 2. Initialize
con-shell init

# 3. Edit configuration
vim .con/cluster.conf

# 4. Generate patches (secrets only)
con-shell generate-patches

# 5. Generate machine configs
con-shell generate-configs

# 6. Boot nodes from Talos ISO

# 7. Discover disks
con-shell discover-disks 10.10.10.21

# 8. Apply configs manually
talosctl apply-config --insecure \
  --nodes 10.10.10.21 \
  --file .con/configs/control-plane-1.yaml

# 9. Bootstrap manually
talosctl bootstrap --nodes 10.10.10.21 --talosconfig=./talosconfig
talosctl kubeconfig --nodes 10.10.10.21 --talosconfig=./talosconfig

# 10. Verify
kubectl get nodes
```

## Commands

### `con-shell init`
Initialize configuration directory.

**Creates:**
- `.con/cluster.conf` - Cluster configuration
- `.con/patches/` - Directory for generated patches
- `.con/configs/` - Directory for generated configs

**Edit `.con/cluster.conf`** to set your node IPs, cluster name, etc.

### `con-shell generate-patches`
Generate SECRET patches only.

**Generates:**
- `.con/patches/ghcr-auth.yaml` - Container registry credentials (SECRET)
- `.con/patches/cilium.yaml` - Cilium CNI manifests (if ciliumValuesFile set)

**Uses committed patches from:**
- `setup/patches/system.yaml` - Kernel, sysctls, kubelet config
- `setup/patches/vip.yaml` - HA virtual IP template
- `setup/patches/storage.yaml` - Storage disk template

**Requirements:**
- `.envhost` file with `GITHUB_USERNAME` and `GHCR_PAT`

### `con-shell generate-configs`
Generate per-node machine configurations.

**Generates:**
- `.con/configs/controlplane.yaml` - Base control plane config
- `.con/configs/worker.yaml` - Base worker config
- `.con/configs/control-plane-1.yaml` - Node-specific configs (with IPs)
- `.con/configs/worker-1.yaml` - Node-specific configs (with IPs)
- `./talosconfig` - CLI configuration

**Combines:**
- Committed patches from `setup/patches/`
- Generated patches from `.con/patches/`
- Per-node network configuration

### `con-shell discover-disks <ip>`
List available disks on a node in maintenance mode.

**Usage:**
```bash
con-shell discover-disks 10.10.10.21
```

**Output:**
```
Device:       /dev/sda
Model:        Samsung SSD 970
Size (Bytes): 1000204886016
Size (GB):    931 GB
Bus Path:     pci0000:00/0000:00:1d.0/0000:71:00.0/nvme/nvme0/nvme0n1
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Found 1 disk(s)
```

**Requirements:**
- Node booted from Talos ISO (maintenance mode)
- Node reachable on network
- Port 50001 accessible

### `con-shell reset <mode>`
Reset nodes to clean state.

**Modes:**

**`pre-con`** - Before conference (insecure)
```bash
con-shell reset pre-con
```
- Uses `--insecure` (no auth needed)
- Resets all nodes defined in cluster.conf
- Fast reset (`--graceful=false`)

**`post-con`** - After conference (authenticated)
```bash
con-shell reset post-con
```
- Uses `--talosconfig` (requires valid config)
- Resets all nodes gracefully
- Drains workloads properly

**`<ip>`** - Single node (authenticated)
```bash
con-shell reset 10.10.10.21
```
- Resets specific node
- Graceful reset

## Configuration

### Nix Configuration

```nix
# flake.nix or dev_shell.nix
services.con_shell.physical = {
  enable = true;
  
  # Cluster info
  clusterName = "prod-cluster";
  clusterEndpoint = "https://10.10.10.11:6443";
  
  # Network
  controlPlaneIPs = [ "10.10.10.21" ];
  workerIPs = [ "10.10.10.22" "10.10.10.23" "10.10.10.24" ];
  vipAddress = "10.10.10.11";
  gateway = "10.10.10.1";
  networkCIDR = "10.10.10.0/24";
  
  # Versions
  talosVersion = "v1.11.0";
  ciliumVersion = "1.16.5";
  
  # Hardware
  installDisk = "/dev/sda";
  
  # Optional: Custom Cilium values
  ciliumValuesFile = ./setup/k8/cilium-values.yaml;
};
```

### Cluster Configuration File

After `con-shell init`, edit `.con/cluster.conf`:

```bash
# Cluster name and endpoint
CLUSTER_NAME="prod-cluster"
CLUSTER_ENDPOINT="https://10.10.10.11:6443"
VIP_IP="10.10.10.11"

# Network
GATEWAY="10.10.10.1"
NETWORK_CIDR="10.10.10.0/24"

# Versions
TALOS_VERSION="v1.11.0"
CILIUM_VERSION="1.16.5"

# Installation
INSTALL_DISK="/dev/sda"

# Node IPs
CONTROL_PLANE_IPS=(10.10.10.21)
WORKER_IPS=(10.10.10.22 10.10.10.23 10.10.10.24)
```

### Environment Variables

Create `.envhost` for secrets:

```bash
# GitHub Container Registry
GITHUB_USERNAME=your-username
GHCR_PAT=ghp_your_personal_access_token

# Docker Hub (optional)
DH_UNAME=your-dockerhub-username
DH_PAT=your-dockerhub-token
```

**Never commit `.envhost`!**

## Manual Operations

### Applying Configurations

```bash
# Apply to control plane
talosctl apply-config --insecure \
  --nodes 10.10.10.21 \
  --file .con/configs/control-plane-1.yaml

# Apply to workers
talosctl apply-config --insecure \
  --nodes 10.10.10.22 \
  --file .con/configs/worker-1.yaml
```

### Bootstrapping Cluster

```bash
# Set endpoints
talosctl --talosconfig=./talosconfig config endpoints 10.10.10.21

# Bootstrap etcd
talosctl bootstrap --nodes 10.10.10.21 --talosconfig=./talosconfig

# Get kubeconfig
talosctl kubeconfig --nodes 10.10.10.21 --talosconfig=./talosconfig

# Verify
kubectl get nodes
```

### Health Checks

```bash
# Talos health
talosctl --talosconfig=./talosconfig health

# Node status
kubectl get nodes -o wide

# Pod status
kubectl get pods -A

# Cilium status
cilium status
```

## Patches

### Committed Patches (No Secrets)

Located in `setup/patches/`:

**`system.yaml`** - Core system configuration
```yaml
machine:
  time:
    bootTimeout: 2m
  kernel:
    modules:
      - name: br_netfilter
  sysctls:
    net.ipv4.ip_forward: "1"
  kubelet:
    extraArgs:
      rotate-server-certificates: "true"
```

**`vip.yaml`** - Virtual IP template (for HA)

**`storage.yaml`** - Storage disk template (for Rook/Ceph)

### Generated Patches (Secrets)

Located in `.con/patches/`:

**`ghcr-auth.yaml`** - Container registry credentials
```yaml
machine:
  registries:
    config:
      ghcr.io:
        auth:
          auth: "base64-encoded-credentials"
```

**`cilium.yaml`** - Cilium CNI manifests (large, ~10k lines)

### Customizing Patches

**Non-sensitive changes:** Edit `setup/patches/*.yaml` and commit

**Sensitive changes:** Handled automatically from `.envhost`

## Directory Structure

```
.con/                           # Generated (not committed)
├── cluster.conf                # Your cluster config
├── patches/
│   ├── ghcr-auth.yaml         # SECRET - registry auth
│   └── cilium.yaml            # Generated from Helm
└── configs/
    ├── controlplane.yaml      # Base configs
    ├── worker.yaml
    ├── control-plane-1.yaml   # Per-node configs
    ├── worker-1.yaml
    └── talosconfig            # CLI config

setup/patches/                  # Committed (no secrets)
├── README.md
├── system.yaml                # System configuration
├── vip.yaml                   # VIP template
└── storage.yaml               # Storage template

./talosconfig                  # CLI config (copied)
```

## Workflows

### Pre-Conference Setup

```bash
# Day before
1. con-shell reset pre-con           # Clean all nodes
2. Power on nodes
3. Boot from Talos ISO
4. Verify IPs assigned (10.10.10.21-24)

# Day of conference
5. con-shell discover-disks 10.10.10.21
6. Apply configs with talosctl
7. Bootstrap with talosctl
8. Deploy workshop materials
```

### Post-Conference Cleanup

```bash
1. con-shell reset post-con          # Graceful reset
2. Power off nodes
3. Done - nodes are clean
```

### Single Node Issues

```bash
1. con-shell reset 10.10.10.23       # Reset problem node
2. Boot from ISO
3. talosctl apply-config...          # Reapply
4. Node rejoins cluster
```

## Troubleshooting

### Node not booting

```bash
# Check console output
# Verify ISO boot order in BIOS/UEFI
# Check network connectivity
```

### Can't discover disks

```bash
# Ensure node booted from ISO
ping 10.10.10.21

# Check maintenance port
nc -zv 10.10.10.21 50001

# Try direct talosctl
talosctl -n 10.10.10.21 --insecure get disks
```

### Config apply fails

```bash
# Verify node is in maintenance mode
talosctl -n 10.10.10.21 --insecure version

# Check network
ping 10.10.10.21

# Verify config file
cat .con/configs/control-plane-1.yaml
```

### Bootstrap fails

```bash
# Wait 2-3 minutes after apply
# Check node is fully booted
talosctl --talosconfig=./talosconfig --nodes 10.10.10.21 version

# Verify etcd not already running
talosctl --talosconfig=./talosconfig --nodes 10.10.10.21 service etcd status
```

### Patches not applied

```bash
# Check patch order in generated config
grep -A 50 "machine:" .con/configs/control-plane-1.yaml

# Verify patches exist
ls -la setup/patches/
ls -la .con/patches/
```

## Best Practices

### ✅ DO
- Reset nodes before conference (`pre-con`)
- Discover disks to verify hardware
- Apply configs one node at a time
- Test one node before applying to all
- Document actual disk paths found
- Keep `.envhost` secure (never commit)

### ❌ DON'T
- Skip the reset step
- Assume disk paths without checking
- Apply to all nodes simultaneously
- Commit `.envhost` or `.con/patches/`
- Use `post-con` reset during conference
- Modify committed patches with secrets

## FAQ

**Q: Why no automatic apply?**
A: Manual control gives you visibility and control. You see exactly what's happening.

**Q: Why generate patches if they're committed?**
A: Only SECRET patches are generated. Everything else is committed for transparency.

**Q: Can I automate the apply step?**
A: Yes, script it yourself with a loop calling `talosctl apply-config` for each node.

**Q: What if my disk path is different?**
A: Use `con-shell discover-disks` to find it, then edit `.con/cluster.conf` `INSTALL_DISK`.

**Q: Do I need VIP for single control plane?**
A: No, but set `vipAddress` to your control plane IP in the config.

**Q: How do I update Cilium version?**
A: Change `ciliumVersion` in your Nix config or `.con/cluster.conf`, regenerate patches.

**Q: Can I use different values for dev and prod?**
A: Yes, use different `ciliumValuesFile` paths in your Nix configs.

## Related Documentation

- [Talos Getting Started](https://www.talos.dev/latest/introduction/getting-started/)
- [Cilium Installation](https://docs.cilium.io/en/stable/installation/)
- [Patches Refactor](./patches-refactor-summary.md)
- [Setup Patches README](../setup/patches/README.md)