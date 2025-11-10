# =============================================================================
# nix/con_shell/config_generators.nix
# Scripts for generating Talos machine configurations
# =============================================================================
{ pkgs, lib, conConfig }:
let
  generateConfigsScript = pkgs.writeShellApplication {
    name = "con-generate-configs";
    runtimeInputs = with pkgs; [ talosctl coreutils gnused gawk ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      PATCHES_DIR="''${CONFIG_DIR}/patches"
      CONFIGS_DIR="''${CONFIG_DIR}/configs"
      
      echo "🔧 Generating machine configurations..."
      echo ""
      
      # Load cluster config
      if [ ! -f "''${CONFIG_DIR}/cluster.conf" ]; then
        echo "✗ Cluster config not found"
        echo "ℹ Run: con-shell init"
        exit 1
      fi
      
      # shellcheck source=/dev/null
      source "''${CONFIG_DIR}/cluster.conf"
      
      # Ensure patches exist
      if [ ! -d "''${PATCHES_DIR}" ]; then
        echo "✗ Patches directory not found"
        echo "ℹ Run: con-shell generate-patches"
        exit 1
      fi
      
      # Count patch files (excluding cilium-values.yaml)
      PATCH_COUNT=$(find "''${PATCHES_DIR}" -name "*.yaml" ! -name "cilium-values.yaml" 2>/dev/null | wc -l)
      
      if [ "''${PATCH_COUNT}" -eq 0 ]; then
        echo "✗ No patches found"
        echo "ℹ Run: con-shell generate-patches"
        exit 1
      fi
      
      # Build patch arguments
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "Configuration Patches"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      PATCH_ARGS=()
      for patch in "''${PATCHES_DIR}"/*.yaml; do
        if [ -f "''${patch}" ] && [ "$(basename "''${patch}")" != "cilium-values.yaml" ]; then
          PATCH_ARGS+=("--config-patch" "@''${patch}")
          echo "✓ $(basename "''${patch}")"
        fi
      done
      echo ""
      
      # Generate base configs
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "Generating Base Configurations"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "ℹ Cluster: ''${CLUSTER_NAME}"
      echo "ℹ Endpoint: ''${CLUSTER_ENDPOINT}"
      echo ""
      
      talosctl gen config "''${CLUSTER_NAME}" "''${CLUSTER_ENDPOINT}" \
        --output-dir "''${CONFIGS_DIR}" \
        "''${PATCH_ARGS[@]}"
      
      echo "✓ Base configurations generated"
      echo ""
      
      # Generate per-node configs for control planes
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "Control Plane Node Configurations"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      NODE_NUM=1
      for ip in "''${CONTROL_PLANE_IPS[@]}"; do
        NODE_NAME="control-plane-''${NODE_NUM}"
        NODE_PATCH="''${CONFIGS_DIR}/''${NODE_NAME}-patch.yaml"
        
        cat > "''${NODE_PATCH}" << EOF
      machine:
        network:
          hostname: ''${NODE_NAME}
          interfaces:
            - interface: eth0
              dhcp: false
              addresses:
                - ''${ip}/24
              routes:
                - network: 0.0.0.0/0
                  gateway: ''${GATEWAY}
              vip:
                ip: ''${VIP_IP}
      EOF
        
        talosctl machineconfig patch "''${CONFIGS_DIR}/controlplane.yaml" \
          --patch @"''${NODE_PATCH}" \
          --output "''${CONFIGS_DIR}/''${NODE_NAME}.yaml"
        
        echo "✓ ''${NODE_NAME} (''${ip})"
        ((NODE_NUM++))
      done
      echo ""
      
      # Generate per-node configs for workers
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "Worker Node Configurations"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      NODE_NUM=1
      for ip in "''${WORKER_IPS[@]}"; do
        NODE_NAME="worker-''${NODE_NUM}"
        NODE_PATCH="''${CONFIGS_DIR}/''${NODE_NAME}-patch.yaml"
        
        cat > "''${NODE_PATCH}" << EOF
      machine:
        network:
          hostname: ''${NODE_NAME}
          interfaces:
            - interface: eth0
              dhcp: false
              addresses:
                - ''${ip}/24
              routes:
                - network: 0.0.0.0/0
                  gateway: ''${GATEWAY}
      EOF
        
        talosctl machineconfig patch "''${CONFIGS_DIR}/worker.yaml" \
          --patch @"''${NODE_PATCH}" \
          --output "''${CONFIGS_DIR}/''${NODE_NAME}.yaml"
        
        echo "✓ ''${NODE_NAME} (''${ip})"
        ((NODE_NUM++))
      done
      echo ""
      
      # Copy talosconfig to current directory
      cp "''${CONFIGS_DIR}/talosconfig" ./talosconfig
      
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "✅ Configuration Generation Complete!"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo ""
      echo "📁 Files generated:"
      echo "  • Base configs: ''${CONFIGS_DIR}/"
      echo "  • Per-node configs: ''${CONFIGS_DIR}/<node-name>.yaml"
      echo "  • Talos config: ./talosconfig"
      echo ""
      echo "📝 Next steps:"
      echo "  1. Boot your physical nodes with Talos ISO"
      echo "  2. Ensure nodes receive IPs: ''${CONTROL_PLANE_IPS[*]} ''${WORKER_IPS[*]}"
      echo "  3. Run: con-shell apply"
      echo "  4. Wait 2-3 minutes for installation"
      echo "  5. Run: con-shell bootstrap"
    '';
  };

in
{
  inherit generateConfigsScript;
}