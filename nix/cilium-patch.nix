{ config, lib, name, pkgs, ... }:

let
  inherit (lib) types mkOption;

  values = builtins.readFile ./setup/k8/cilium-values.yaml;

  # Fetch the Cilium Helm chart directly
  ciliumChart = pkgs.fetchurl {
    url = "https://github.com/cilium/charts/releases/download/cilium-1.16.5/cilium-1.16.5.tgz";
    sha256 = lib.fakeSha256; # You'll need to update this
  };

  # This creates a shell script in the Nix store that contains all our commands.
  setupScript = pkgs.writeShellApplication {
    name = "create_cilium_patch";
    
    # This ensures helm and kubectl are available in the script's PATH at runtime.
    runtimeInputs = [
      config.kubectlPackage
      config.helmPackage
    ];

    text = ''
      set -euo pipefail

      OUTPUT_FILE="${config.dataDir}/cilium.yaml"

      echo "Generating Cilium manifests from local chart..."
      CILIUM_CHART=$(helm template cilium ${ciliumChart} \
        --namespace kube-system \
        --values ${(lib.escapeShellArg values)})

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

      echo "$CILIUM_CHART" | sed 's/^/        /' >> "$OUTPUT_FILE"

      echo "✓ Patch written to $OUTPUT_FILE"
    '';
  };

in
{
  options = {
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