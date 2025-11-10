# =============================================================================
# nix/dev_patches.nix
# Development-specific patches for QEMU Talos cluster
# Uses con_shell patch generators with dev-specific overrides
# =============================================================================
{ pkgs, lib, config, name, ... }:
let
  inherit (lib) types mkOption mkIf;

  # Import con_shell patch generators
  conShellPatchGenerators = import ./con_shell/patch_generators.nix {
    inherit pkgs lib;
    conConfig = config;
    ciliumValuesFile = config.ciliumValuesFile;
  };

  # Script to generate all dev patches
  generateDevPatchesScript = pkgs.writeShellApplication {
    name = "generate-dev-patches";
    runtimeInputs = with pkgs; [ 
      coreutils 
      kubernetes-helm
      gnused
    ];
    
    text = ''
      set -euo pipefail
      
      PATCHES_DIR="${config.dataDir}"
      export CONFIG_DIR="${config.dataDir}"
      
      echo "🔧 Generating development patches..."
      mkdir -p "''${PATCHES_DIR}"
      
      # 1. Generate Cilium patch using con_shell generator
      echo ""
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "1/3: Cilium CNI (Development)"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      ${lib.getExe conShellPatchGenerators.ciliumPatchScript} ${
        if config.ciliumValuesFile != null 
        then ''"${config.ciliumValuesFile}"'' 
        else ""
      }
      
      # Move to correct location for Talos
      mv "''${PATCHES_DIR}/patches/cilium.yaml" "''${PATCHES_DIR}/cilium.yaml" || true
      rm -rf "''${PATCHES_DIR}/patches" || true
      
      # 2. Generate GHCR auth patch
      echo ""
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "2/3: Container Registry Auth (Development)"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      ${lib.getExe conShellPatchGenerators.ghcrPatchScript}
      
      # Move to correct location
      mv "''${PATCHES_DIR}/patches/ghcr-auth.yaml" "''${PATCHES_DIR}/ghcr.yaml" || true
      
      # 3. Generate dev-specific system patch
      echo ""
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      echo "3/3: Development System Config"
      echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
      
      cat > "''${PATCHES_DIR}/dev-system.yaml" << 'EOF'
      machine:
        time:
          bootTimeout: 2m
        
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
              - 10.5.0.0/24
      EOF
      
      echo "✓ Dev system patch created"
      
      # Cleanup temp directories
      rm -rf "''${PATCHES_DIR}/patches" || true
      rm -rf "''${PATCHES_DIR}/configs" || true
      
      echo ""
      echo "✅ All development patches generated!"
      echo ""
      echo "📁 Generated patches:"
      ls -lh "''${PATCHES_DIR}"/*.yaml 2>/dev/null || echo "  (none found)"
      echo ""
      echo "📝 These patches will be applied to the Talos cluster on boot"
    '';
  };

in
{
  options = {
    ciliumVersion = mkOption {
      type = types.str;
      default = "1.16.5";
      description = "Cilium version for development cluster";
    };

    ciliumValuesFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "Path to Cilium values file. If null, uses default values.";
      example = ./setup/k8/cilium-values.yaml;
    };

    dataDir = mkOption {
      type = types.str;
      default = ".data/talos-patches";
      description = "Directory for generated patches";
    };
  };

  config = mkIf config.enable {
    outputs.settings.processes = {
      "${name}" = {
        command = generateDevPatchesScript;
        environment = {
          CILIUM_VERSION = config.ciliumVersion;
        };
      };
    };
  };
}