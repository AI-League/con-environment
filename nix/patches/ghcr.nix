# nix/con_shell/ghcr-patch.nix
#
# This function builds a script that renders the
# GHCR auth patch (a secret) to a file.
{
  pkgs,
  outputFile # Required: The root config dir (e.g., ".con")
}:

pkgs.writeShellApplication {
  name = "generate-ghcr-patch";
  runtimeInputs = [ pkgs.coreutils ]; # Provides 'base64'
  
  text = ''
    set -euo pipefail
    
    # Path is baked in by Nix
    
    # All status messages go to stderr (>&2)
    echo "🔧 Generating GHCR authentication patch..." >&2
    
    # Ensure the output directory exists
    mkdir -p "$(dirname "${outputFile}")"

    # Check for credentials in the environment (SECRETS)
    # This logic correctly remains at runtime.
    if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
      echo "⚠ GITHUB_USERNAME and GHCR_PAT not set in environment" >&2
      echo "ℹ Loading from .envhost if available..." >&2
      
      if [ -f .envhost ]; then
        set -a
        # shellcheck source=/dev/null
        source .envhost
        set +a
      fi
      
      if [ -z "''${GITHUB_USERNAME:-}" ] || [ -z "''${GHCR_PAT:-}" ]; then
        echo "✗ GITHUB_USERNAME and GHCR_PAT must be set" >&2
        echo "ℹ Create a .envhost file with:" >&2
        echo "  GITHUB_USERNAME=your-username" >&2
        echo "  GHCR_PAT=your-personal-access-token" >&2
        exit 1
      fi
    fi
    
    AUTH_STRING=$(echo -n "''${GITHUB_USERNAME}:''${GHCR_PAT}" | base64 -w 0)
    
    # Write the final YAML patch *directly* to the output file
    cat > "${outputFile}" << EOF
machine:
  registries:
    config:
      ghcr.io:
        auth:
          auth: "''${AUTH_STRING}"
    time:
      bootTimeout: 2m
EOF

    echo "✓ GHCR authentication patch generated: ${outputFile}" >&2
  '';
}