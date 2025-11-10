# =============================================================================
# nix/con_shell/patch_generators.nix
# Scripts for generating SECRET patches only
# Non-sensitive patches should be committed in setup/patches/
# =============================================================================
{ pkgs, lib, conConfig, ciliumValuesFile }:
let
  # Initialize configuration directory
  initScript = pkgs.writeShellApplication {
    name = "con-init";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      CONFIGS_DIR="''${CONFIG_DIR}/configs"
      
      echo "🔧 Initializing configuration directory..."
      
      mkdir -p "''${PATCHES_DIR}"
      mkdir -p "''${CONFIGS_DIR}"
      
      # Create cluster config file
      cat > "''${CONFIG_DIR}/cluster.conf" << 'EOF'
      # Physical Cluster Configuration
      # Edit this file to customize your cluster setup
      
      CLUSTER_NAME="${conConfig.clusterName}"
      CLUSTER_ENDPOINT="${conConfig.clusterEndpoint}"
      VIP_IP="${conConfig.vipAddress}"
      GATEWAY="${conConfig.gateway}"
      NETWORK_CIDR="${conConfig.networkCIDR}"
      
      TALOS_VERSION="${conConfig.talosVersion}"
      CILIUM_VERSION="${conConfig.ciliumVersion}"
      INSTALL_DISK="${conConfig.installDisk}"
      
      # Node IPs - Edit these arrays as needed
      CONTROL_PLANE_IPS=(${lib.concatStringsSep " " conConfig.controlPlaneIPs})
      WORKER_IPS=(${lib.concatStringsSep " " conConfig.workerIPs})
      EOF
      
      echo "✓ Configuration directory initialized at ''${CONFIG_DIR}"
      echo ""
      echo "📝 Next steps:"
      echo "  1. Edit ''${CONFIG_DIR}/cluster.conf to customize settings"
      echo "  2. Run: con-shell generate-patches"
    '';
  };

  # Generate Cilium patch (if ciliumValuesFile provided)
  ciliumPatchScript = pkgs.writeShellApplication {
    name = "con-generate-cilium-patch";
    runtimeInputs = with pkgs; [ kubernetes-helm gnused coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      # Allow passing values file as argument
      VALUES_FILE="''${1:-}"
      
      # Load cluster config if available
      if [ -f "''${CONFIG_DIR}/cluster.conf" ]; then
        # shellcheck source=/dev/null
        source "''${CONFIG_DIR}/cluster.conf"
      fi
      
      CILIUM_VERSION="''${CILIUM_VERSION:-${conConfig.ciliumVersion}}"
      
      echo "🔧 Generating Cilium patch..."
      
      # Determine which values file to use
      if [ -z "''${VALUES_FILE}" ]; then
        echo "✗ No Cilium values file provided"
        echo "ℹ Usage: con-generate-cilium-patch <path-to-values.yaml>"
        echo "ℹ Example: con-generate-cilium-patch ./setup/k8/cilium-values.yaml"
        exit 1
      fi
      
      if [ ! -f "''${VALUES_FILE}" ]; then
        echo "✗ Values file not found: ''${VALUES_FILE}"
        exit 1
      fi
      
      echo "ℹ Using values file: ''${VALUES_FILE}"
      CILIUM_VALUES_FILE="''${VALUES_FILE}"
      
      echo "ℹ Adding Cilium Helm repository..."
      helm repo add cilium https://helm.cilium.io/ 2>/dev/null || true
      helm repo update cilium 2>/dev/null
      
      echo "ℹ Generating Cilium manifests (version ''${CILIUM_VERSION})..."
      CILIUM_MANIFESTS=$(helm template cilium cilium/cilium \
        --version "''${CILIUM_VERSION}" \
        --namespace kube-system \
        --values "''${CILIUM_VALUES_FILE}")
      
      # Create Talos patch
      cat > "''${PATCHES_DIR}/cilium.yaml" << 'PATCH_START'
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
      PATCH_START
      
      # Add manifests with proper indentation
      echo "''${CILIUM_MANIFESTS}" | sed 's/^/        /' >> "''${PATCHES_DIR}/cilium.yaml"
      
      echo "✓ Cilium patch generated: ''${PATCHES_DIR}/cilium.yaml"
      echo "ℹ Values used: ''${CILIUM_VALUES_FILE}"
    '';
  };

  # Generate GHCR authentication patch (SECRETS ONLY)
  ghcrPatchScript = pkgs.writeShellApplication {
    name = "con-generate-ghcr-patch";
    runtimeInputs = with pkgs; [ coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      echo "🔧 Generating GHCR authentication patch..."
      
      # Check for credentials
      if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
        echo "⚠ GITHUB_USERNAME and GHCR_PAT not set in environment"
        echo "ℹ Loading from .envhost if available..."
        
        if [ -f .envhost ]; then
          set -a
          # shellcheck source=/dev/null
          source .envhost
          set +a
        fi
        
        if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
          echo "✗ GITHUB_USERNAME and GHCR_PAT must be set"
          echo "ℹ Create a .envhost file with:"
          echo "  GITHUB_USERNAME=your-username"
          echo "  GHCR_PAT=your-personal-access-token"
          exit 1
        fi
      fi
      
      AUTH_STRING=$(echo -n "''${GITHUB_USERNAME}:''${GHCR_PAT}" | base64 -w 0)
      
      cat > "''${PATCHES_DIR}/ghcr-auth.yaml" << EOF
      machine:
        registries:
          config:
            ghcr.io:
              auth:
                auth: "''${AUTH_STRING}"
        time:
          bootTimeout: 2m
      EOF
      
      echo "✓ GHCR authentication patch generated: ''${PATCHES_DIR}/ghcr-auth.yaml"
    '';
  };

  # VIP patch generation removed - should be in setup/patches/vip.yaml
  vipPatchScript = pkgs.writeShellApplication {
    name = "con-generate-vip-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      echo "⚠ VIP patch should be committed in setup/patches/vip.yaml"
      echo "ℹ This script is deprecated - VIP configuration contains no secrets"
      exit 1
    '';
  };

  # System patch generation removed - should be in setup/patches/system.yaml
  systemPatchScript = pkgs.writeShellApplication {
    name = "con-generate-system-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      echo "⚠ System patch should be committed in setup/patches/system.yaml"
      echo "ℹ This script is deprecated - system configuration contains no secrets"
      exit 1
    '';
  };

  # Storage patch generation removed - should be in setup/patches/storage.yaml
  storagePatchScript = pkgs.writeShellApplication {
    name = "con-generate-storage-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      echo "⚠ Storage patch should be committed in setup/patches/storage.yaml"
      echo "ℹ This script is deprecated - storage configuration contains no secrets"
      exit 1
    '';
  };

  # Generate only secret patches
  generateAllScript = pkgs.writeShellApplication {
    name = "con-generate-all-patches";
    runtimeInputs = [ 
      ciliumPatchScript 
      ghcrPatchScript 
    ];
    
    text = ''
      set -euo pipefail
      
      echo "🔧 Generating SECRET patches..."
      echo ""
      echo "ℹ Non-sensitive patches should be committed in setup/patches/"
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "1/2: Cilium CNI Configuration"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      ${if ciliumValuesFile != null then 
        ''con-generate-cilium-patch "${ciliumValuesFile}"'' 
      else 
        ''
        echo "⚠ No Cilium values file configured"
        echo "ℹ Skipping Cilium patch generation"
        echo "ℹ Set ciliumValuesFile in your Nix config to auto-generate"
        ''}
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "2/2: Container Registry Authentication (SECRET)"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      con-generate-ghcr-patch
      echo ""
      
      echo "✅ Secret patches generated!"
      echo ""
      echo "📝 Next steps:"
      echo "  1. Review generated patches in .con/patches/"
      echo "  2. Ensure non-sensitive patches exist in setup/patches/"
      echo "  3. Run: con-shell generate-configs"
    '';
  };

in
{
  inherit initScript;
  inherit ciliumPatchScript;
  inherit ghcrPatchScript;
  inherit systemPatchScript;
  inherit vipPatchScript;
  inherit storagePatchScript;
  inherit generateAllScript;
}

  # Initialize configuration directory
  initScript = pkgs.writeShellApplication {
    name = "con-init";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      CONFIGS_DIR="''${CONFIG_DIR}/configs"
      
      echo "🔧 Initializing configuration directory..."
      
      mkdir -p "''${PATCHES_DIR}"
      mkdir -p "''${CONFIGS_DIR}"
      
      # Create cluster config file
      cat > "''${CONFIG_DIR}/cluster.conf" << 'EOF'
      # Physical Cluster Configuration
      # Edit this file to customize your cluster setup
      
      CLUSTER_NAME="${conConfig.clusterName}"
      CLUSTER_ENDPOINT="${conConfig.clusterEndpoint}"
      VIP_IP="${conConfig.vipAddress}"
      GATEWAY="${conConfig.gateway}"
      NETWORK_CIDR="${conConfig.networkCIDR}"
      
      TALOS_VERSION="${conConfig.talosVersion}"
      CILIUM_VERSION="${conConfig.ciliumVersion}"
      INSTALL_DISK="${conConfig.installDisk}"
      
      # Node IPs - Edit these arrays as needed
      CONTROL_PLANE_IPS=(${lib.concatStringsSep " " conConfig.controlPlaneIPs})
      WORKER_IPS=(${lib.concatStringsSep " " conConfig.workerIPs})
      EOF
      
      echo "✓ Configuration directory initialized at ''${CONFIG_DIR}"
      echo ""
      echo "📝 Next steps:"
      echo "  1. Edit ''${CONFIG_DIR}/cluster.conf to customize settings"
      echo "  2. Run: con-shell generate-patches"
    '';
  };

  # Generate Cilium patch
  ciliumPatchScript = pkgs.writeShellApplication {
    name = "con-generate-cilium-patch";
    runtimeInputs = with pkgs; [ kubernetes-helm gnused coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      # Allow passing values file as argument
      VALUES_FILE="''${1:-}"
      
      # Load cluster config if available
      if [ -f "''${CONFIG_DIR}/cluster.conf" ]; then
        # shellcheck source=/dev/null
        source "''${CONFIG_DIR}/cluster.conf"
      fi
      
      CILIUM_VERSION="''${CILIUM_VERSION:-${conConfig.ciliumVersion}}"
      
      echo "🔧 Generating Cilium patch..."
      
      # Determine which values file to use
      if [ -n "''${VALUES_FILE}" ]; then
        if [ ! -f "''${VALUES_FILE}" ]; then
          echo "✗ Values file not found: ''${VALUES_FILE}"
          exit 1
        fi
        echo "ℹ Using provided values file: ''${VALUES_FILE}"
        CILIUM_VALUES_FILE="''${VALUES_FILE}"
      else
        echo "ℹ Creating default Cilium values file..."
        CILIUM_VALUES_FILE="''${PATCHES_DIR}/cilium-values.yaml"
        
        # Create default Cilium values file
        cat > "''${CILIUM_VALUES_FILE}" << 'EOF'
      ipam:
        mode: kubernetes
      
      kubeProxyReplacement: true
      
      securityContext:
        capabilities:
          ciliumAgent:
            - CHOWN
            - KILL
            - NET_ADMIN
            - NET_RAW
            - IPC_LOCK
            - SYS_ADMIN
            - SYS_RESOURCE
            - DAC_OVERRIDE
            - FOWNER
            - SETGID
            - SETUID
          cleanCiliumState:
            - NET_ADMIN
            - SYS_ADMIN
            - SYS_RESOURCE
      
      cgroup:
        autoMount:
          enabled: false
        hostRoot: /sys/fs/cgroup
      
      k8sServiceHost: localhost
      k8sServicePort: 7445
      
      hubble:
        enabled: true
        relay:
          enabled: true
        ui:
          enabled: true
      
      tunnelProtocol: vxlan
      
      cni:
        chainingMode: none
        exclusive: true
      
      gatewayAPI:
        enabled: true
        enableAlpn: true
        enableAppProtocol: true
      EOF
        echo "✓ Default values file created: ''${CILIUM_VALUES_FILE}"
      fi
      
      echo "ℹ Adding Cilium Helm repository..."
      helm repo add cilium https://helm.cilium.io/ 2>/dev/null || true
      helm repo update cilium 2>/dev/null
      
      echo "ℹ Generating Cilium manifests (version ''${CILIUM_VERSION})..."
      CILIUM_MANIFESTS=$(helm template cilium cilium/cilium \
        --version "''${CILIUM_VERSION}" \
        --namespace kube-system \
        --values "''${CILIUM_VALUES_FILE}")
      
      # Create Talos patch
      cat > "''${PATCHES_DIR}/cilium.yaml" << 'PATCH_START'
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
      PATCH_START
      
      # Add manifests with proper indentation
      echo "''${CILIUM_MANIFESTS}" | sed 's/^/        /' >> "''${PATCHES_DIR}/cilium.yaml"
      
      echo "✓ Cilium patch generated: ''${PATCHES_DIR}/cilium.yaml"
      echo "ℹ Values used: ''${CILIUM_VALUES_FILE}"
    '';
  };

  # Generate GHCR authentication patch
  ghcrPatchScript = pkgs.writeShellApplication {
    name = "con-generate-ghcr-patch";
    runtimeInputs = with pkgs; [ coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      echo "🔧 Generating GHCR authentication patch..."
      
      # Check for credentials
      if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
        echo "⚠ GITHUB_USERNAME and GHCR_PAT not set in environment"
        echo "ℹ Loading from .envhost if available..."
        
        if [ -f .envhost ]; then
          set -a
          # shellcheck source=/dev/null
          source .envhost
          set +a
        fi
        
        if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
          echo "✗ GITHUB_USERNAME and GHCR_PAT must be set"
          echo "ℹ Create a .envhost file with:"
          echo "  GITHUB_USERNAME=your-username"
          echo "  GHCR_PAT=your-personal-access-token"
          exit 1
        fi
      fi
      
      AUTH_STRING=$(echo -n "''${GITHUB_USERNAME}:''${GHCR_PAT}" | base64 -w 0)
      
      cat > "''${PATCHES_DIR}/ghcr-auth.yaml" << EOF
      machine:
        registries:
          config:
            ghcr.io:
              auth:
                auth: "''${AUTH_STRING}"
        time:
          bootTimeout: 2m
      EOF
      
      echo "✓ GHCR authentication patch generated: ''${PATCHES_DIR}/ghcr-auth.yaml"
    '';
  };

  # Generate system configuration patch
  systemPatchScript = pkgs.writeShellApplication {
    name = "con-generate-system-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      # Load cluster config if available
      if [ -f "''${CONFIG_DIR}/cluster.conf" ]; then
        # shellcheck source=/dev/null
        source "''${CONFIG_DIR}/cluster.conf"
      fi
      
      INSTALL_DISK="''${INSTALL_DISK:-${conConfig.installDisk}}"
      TALOS_VERSION="''${TALOS_VERSION:-${conConfig.talosVersion}}"
      NETWORK_CIDR="''${NETWORK_CIDR:-${conConfig.networkCIDR}}"
      
      echo "🔧 Generating system patch..."
      
      cat > "''${PATCHES_DIR}/system.yaml" << EOF
      machine:
        install:
          disk: ''${INSTALL_DISK}
          image: ghcr.io/siderolabs/installer:''${TALOS_VERSION}
          wipe: false
        
        kernel:
          modules:
            - name: br_netfilter
              parameters:
                - nf_conntrack_max=131072
        
        sysctls:
          net.bridge.bridge-nf-call-iptables: "1"
          net.bridge.bridge-nf-call-ip6tables: "1"
          net.ipv4.ip_forward: "1"
          net.ipv6.conf.all.forwarding: "1"
        
        kubelet:
          extraArgs:
            rotate-server-certificates: "true"
          nodeIP:
            validSubnets:
              - ''${NETWORK_CIDR}
      EOF
      
      echo "✓ System patch generated: ''${PATCHES_DIR}/system.yaml"
      echo "ℹ Install disk: ''${INSTALL_DISK}"
      echo "ℹ Talos version: ''${TALOS_VERSION}"
    '';
  };

  # Generate VIP patch for HA
  vipPatchScript = pkgs.writeShellApplication {
    name = "con-generate-vip-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      # Load cluster config
      if [ -f "''${CONFIG_DIR}/cluster.conf" ]; then
        # shellcheck source=/dev/null
        source "''${CONFIG_DIR}/cluster.conf"
      fi
      
      VIP_IP="''${VIP_IP:-${conConfig.vipAddress}}"
      
      # Check if we have multiple control planes
      if [ "''${#CONTROL_PLANE_IPS[@]}" -eq 1 ]; then
        echo "ℹ Single control plane node - skipping VIP configuration"
        return 0
      fi
      
      echo "🔧 Generating VIP patch for HA control plane..."
      
      cat > "''${PATCHES_DIR}/vip.yaml" << EOF
      machine:
        network:
          interfaces:
            - interface: eth0
              vip:
                ip: ''${VIP_IP}
      EOF
      
      echo "✓ VIP patch generated: ''${PATCHES_DIR}/vip.yaml"
      echo "ℹ VIP Address: ''${VIP_IP}"
    '';
  };

  # Generate storage patch template
  storagePatchScript = pkgs.writeShellApplication {
    name = "con-generate-storage-patch";
    runtimeInputs = [ pkgs.coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      
      echo "🔧 Generating storage patch template..."
      
      cat > "''${PATCHES_DIR}/storage.yaml" << 'EOF'
      # Storage Configuration Template
      # Edit this file to match your storage setup for Ceph/Rook
      # 
      # This configuration:
      # - Formats /dev/sdb with a single partition mounted at /var/lib/storage
      # - Makes this directory available to containers via kubelet
      # - Enables shared mount propagation for Rook/Ceph
      
      machine:
        disks:
          - device: /dev/sdb
            partitions:
              - mountpoint: /var/lib/storage
        
        kubelet:
          extraMounts:
            - destination: /var/lib/storage
              type: bind
              source: /var/lib/storage
              options:
                - bind
                - rshared
                - rw
      EOF
      
      echo "✓ Storage patch template created: ''${PATCHES_DIR}/storage.yaml"
      echo "⚠ This is a TEMPLATE - edit it to match your storage configuration"
      echo "ℹ Default device: /dev/sdb"
    '';
  };

  # Generate all patches
  generateAllScript = pkgs.writeShellApplication {
    name = "con-generate-all-patches";
    runtimeInputs = [ 
      ciliumPatchScript 
      ghcrPatchScript 
      systemPatchScript 
      vipPatchScript 
      storagePatchScript 
    ];
    
    text = ''
      set -euo pipefail
      
      echo "🔧 Generating all configuration patches..."
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "1/5: Cilium CNI Configuration"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      ${if ciliumValuesFile != null then 
        ''con-generate-cilium-patch "${ciliumValuesFile}"'' 
      else 
        ''con-generate-cilium-patch''}
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "2/5: Container Registry Authentication"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      con-generate-ghcr-patch
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "3/5: System Configuration"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      con-generate-system-patch
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "4/5: High Availability VIP"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      con-generate-vip-patch
      echo ""
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "5/5: Storage Configuration Template"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      con-generate-storage-patch
      echo ""
      
      echo "✅ All patches generated!"
      echo ""
      echo "📝 Next steps:"
      echo "  1. Review patches in .con/patches/"
      echo "  2. Edit .con/patches/storage.yaml for your storage setup"
      echo "  3. Run: con-shell generate-configs"
    '';
  };

in
{
  inherit initScript;
  inherit ciliumPatchScript;
  inherit ghcrPatchScript;
  inherit systemPatchScript;
  inherit vipPatchScript;
  inherit storagePatchScript;
  inherit generateAllScript;
}