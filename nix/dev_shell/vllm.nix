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
in
{
  options = {
    image = mkOption {
      type = types.str;
      default = "vllm/vllm-openai:latest";
      description = "vLLM Docker image to use";
    };

    model = mkOption {
      type = types.str;
      default = "meta-llama/Llama-3.2-1B-Instruct";
      example = "meta-llama/Meta-Llama-3-8B-Instruct";
      description = "HuggingFace model to serve";
    };

    port = mkOption {
      type = types.port;
      default = 8000;
      description = "Port to expose vLLM API on";
    };

    containerName = mkOption {
      type = types.str;
      default = "vllm-${name}";
      description = "Docker container name";
    };

    enableGpu = mkOption {
      type = types.bool;
      default = true;
      description = "Enable GPU acceleration (requires nvidia-docker)";
    };

    vllmArgs = mkOption {
      type = types.listOf types.str;
      default = [
        "--dtype" "auto"
        "--max-model-len" "4096"
        "--gpu-memory-utilization" "0.9"
      ];
      description = "Additional vLLM arguments";
      example = [
        "--dtype" "float16"
        "--max-model-len" "8192"
        "--tensor-parallel-size" "2"
      ];
    };

    extraEnv = mkOption {
      type = types.listOf types.str;
      default = [];
      description = "Additional environment variables";
      example = [
        "VLLM_LOGGING_LEVEL=DEBUG"
        "TRANSFORMERS_VERBOSITY=debug"
      ];
    };

    waitForReady = mkOption {
      type = types.bool;
      default = true;
      description = "Wait for vLLM to be ready before marking process as started";
    };
  };

  config = mkIf config.enable {
    outputs.settings.processes."${name}" = {
      command = startCommand;
    };
  };
}