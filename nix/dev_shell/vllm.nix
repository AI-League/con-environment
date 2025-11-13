# nix/dev_shell/vllm.nix
# vLLM Service Module for process-compose
{ pkgs, lib, config, name, ... }:
let
  inherit (lib) types mkOption mkIf;

  # Script to start vLLM container
  startCommand = 
    let
      baseArgs = [
        "docker"
        "run"
        "--rm"
        "--name" config.containerName
        "-p" "${toString config.port}:8000"
        "-e" "HUGGING_FACE_HUB_TOKEN=$HUGGINGFACE_TOKEN"
        "-v" "${config.dataDir}:/root/.cache/huggingface"
        "--ipc=host"
      ];
      
      gpuArgs = lib.optionals config.enableGpu [ "--gpus" "all" ];
      
      envArgs = lib.concatMap (env: [ "-e" env ]) config.extraEnv;
      
      modelArgs = [
        config.image
        "--model" config.model
      ] ++ config.vllmArgs;
      
      allArgs = baseArgs ++ gpuArgs ++ envArgs ++ modelArgs;
    in
    pkgs.writeShellApplication {
      name = "start-vllm";
      runtimeInputs = with pkgs; [ docker coreutils ];
      
      text = ''
        set -euo pipefail
        
        echo "🚀 Starting vLLM service..."
        
        # Read token from .envhost at runtime
        if [ ! -f .envhost ]; then
          echo "✗ .envhost file not found"
          exit 1
        fi
        
        set -a
        # shellcheck source=/dev/null
        source .envhost
        set +a
        
        if [ -z "''${HUGGINGFACE_TOKEN:-}" ]; then
          echo "✗ HUGGINGFACE_TOKEN not found in .envhost"
          exit 1
        fi
        
        # Ensure data directory exists
        mkdir -p "${config.dataDir}"
        
        echo "ℹ Model: ${config.model}"
        echo "ℹ Port: ${toString config.port}"
        echo "ℹ GPU: ${if config.enableGpu then "Enabled" else "Disabled"}"
        echo "ℹ Data dir: ${config.dataDir}"
        echo ""
        
        echo "Executing: ${lib.escapeShellArgs allArgs}"
        echo ""
        
        ${lib.escapeShellArgs allArgs}
      '';
    };

  # Cleanup script
  cleanupScript = ''
    set +e
    echo "🧹 Stopping vLLM container..."
    docker stop "${config.containerName}" 2>/dev/null || true
    echo "✓ vLLM stopped"
  '';

  config = mkIf config.enable {
    outputs.settings.processes."${name}" = {
      command = lib.escapeShellArgs allArgs;
      
      ${lib.optionalString config.waitForReady ''
        ready_log_line = "Application startup complete";
      ''}
    };
  };
}