# =============================================================================
# nix/dev_patches.nix
# Development-specific patches for QEMU Talos cluster
# Uses con_shell patch generators with dev-specific overrides
# =============================================================================
{ pkgs, lib, config, name, ... }:
let
  inherit (lib) types mkOption mkIf;

  # Import con_shell patch generators
  cilium_patch = import ../patch/cilium.nix {
    inherit pkgs;
    values = config.ciliumValuesFile;
    output = config.dataDir + ./cilium.yaml;
  };

  ghcr_patch = import ../patch/ghcr.nix {
    inherit pkgs;
    output = config.dataDir + ./ghcr.yaml;
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
      
      echo "🔧 Generating development patches..."
      mkdir -p "${config.dataDir}"
      ${lib.getExe cilium_patch}
      ${lib.getExe ghcr_patch}
    '';
  };

in
{
  options = {
    ciliumValuesFile = mkOption {
      type = types.path;
      default = ./setup/k8/cilium-values.yaml;
      description = "Path to Cilium values file. If null, uses default values.";
      example = ./setup/k8/cilium-values.yaml;
    };
  };

  config = mkIf config.enable {
    outputs.settings.processes = {
      "${name}" = {
        command = generateDevPatchesScript;
      };
    };
  };
}