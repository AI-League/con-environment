# =============================================================================
# nix/con_shell/cluster_ops.nix
# Scripts for cluster operations (apply, bootstrap, health)
# =============================================================================
{ pkgs, lib, conConfig }:
let
  resetScript = pkgs.writeShellApplication {
    name = "con-reset";
    runtimeInputs = with pkgs; [ talosctl coreutils ];
    
    text = ''
      set -euo pipefail
      
      CONFIG_DIR="''${CONFIG_DIR:-.con}"
      MODE="''${1:-}"
      
      if [ -z "''${MODE}" ]; then
        echo "Usage: con-reset <pre-con|post-con|node IP>"
        echo ""
        echo "  pre-con     Reset all nodes (insecure, before conference)"
        echo "  post-con    Reset all nodes (authenticated, after conference)"  
        echo "  <ip>        Reset specific node (authenticated)"
        exit 1
      fi
      
      # Load cluster config
      if [ ! -f "''${CONFIG_DIR}/cluster.conf" ]; then
        echo "✗ Cluster config not found. Run: con-shell init"
        exit 1
      fi
      
      # shellcheck source=/dev/null
      source "''${CONFIG_DIR}/cluster.conf"
      
      case "''${MODE}" in
        pre-con)
          # Before conference - insecure, all nodes
          echo "🧹 Resetting all nodes (insecure mode)..."
          for ip in "''${CONTROL_PLANE_IPS[@]}" "''${WORKER_IPS[@]}"; do
            echo "Resetting ''${ip}..."
            talosctl reset --nodes "''${ip}" --insecure --graceful=false --wait || true
          done
          ;;
        
        post-con)
          # After conference - authenticated, all nodes
          echo "🧹 Resetting all nodes (authenticated)..."
          for ip in "''${CONTROL_PLANE_IPS[@]}" "''${WORKER_IPS[@]}"; do
            echo "Resetting ''${ip}..."
            talosctl reset --nodes "''${ip}" --talosconfig=./talosconfig --wait || true
          done
          ;;
        
        *)
          # Treat as IP address - reset single node
          NODE_IP="''${MODE}"
          echo "🧹 Resetting node ''${NODE_IP}..."
          talosctl reset --nodes "''${NODE_IP}" --talosconfig=./talosconfig --wait
          ;;
      esac
      
      echo "✓ Reset complete"
    '';
  };

in
{
  inherit applyConfigScript;
  inherit bootstrapScript;
  inherit healthCheckScript;
  inherit resetScript;
}