{ config, lib, name, pkgs, ... }:

let
  inherit (lib) types mkOption;

  # This creates a shell script in the Nix store that contains all our commands.
  setupScript = pkgs.writeShellApplication {
    name = "create_cilium_patch";
    
    # This ensures helm and kubectl are available in the script's PATH at runtime.
    runtimeInputs = [
      config.helmPackage
    ];

    text = ''
      set -euo pipefail

      OUTPUT_FILE="${config.dataDir}/cilium.yaml"

      echo "Generating Cilium manifests from local chart..."
      CILIUM_CHART=$(helm template cilium cilium/cilium \
        --namespace kube-system \
        --version "${config.version}" \
        --values ${(lib.escapeShellArg config.values)})

      mkdir -p ${config.dataDir}
      echo "Creating Talos patch..."
      cat > "$OUTPUT_FILE" <<'EOF'
      # This is stored in .data and not checked in.
      # It is meant to be temporary.
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

      # Add indentation line by line to satisfy shellcheck SC2001
      echo "$CILIUM_CHART" | while IFS= read -r line; do
        echo "        $line" >> "$OUTPUT_FILE"
      done

      echo "✓ Patch written to $OUTPUT_FILE"
    '';
  };

in
{
  options = {
    values = mkOption {
      type = types.path;
      description = "The values to use.";
    };

    version = mkOption {
      type = types.str;
      default = "v1.18.3";
      description = "The values to use.";
    };

    helmPackage = mkOption {
      type = types.package;
      default = pkgs.kubernetes-helm;
      description = "The helm package to use.";
    };
  };

  config = {
    outputs.settings.processes."${name}" = {
      command = "${setupScript}/bin/create_cilium_patch";
    };
  };
}