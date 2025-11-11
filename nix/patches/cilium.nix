{
  pkgs,
  values, 
  outputFile,
}:

let
  nix-kube-generators = pkgs.fetchFromGitHub {
    owner = "farcaller";
    repo = "nix-kube-generators";
    sha256 = "sha256-REPLACEME";
  };

  kubelib = nix-kube-generators.lib { inherit pkgs; };

  renderedCiliumManifests = kubelib.BuildHelmChart "cilium" nixhelm.charts.cilium {
    namespace = "kube-system";
    values = values;
    includeCRDs = true;
  };

in
pkgs.writeShellApplication {
  name = "con-generate-cilium-patch";
  runtimeInputs = with pkgs; [ coreutils gnused ];
  
  text = ''
    set -euo pipefail

    mkdir -p "$(dirname "${outputFile}")"
    
    # Use a subshell to group all output and redirect it once
    (
      cat << 'PATCH_START'
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
    
      sed 's/^/        /' "${renderedCiliumManifests}"
      
    ) > "${outputFile}" # Single redirection to the output file
    
    echo "✓ Cilium patch generated: ${outputFile}" >&2
  '';
}