{ inputs, pkgs, projectRoot, system, ... }:
let 
  inherit (inputs.services-flake.lib) multiService;

  cliTools = with pkgs; [
    curl
    talosctl
    kubectl
    kubernetes-helm
    tilt
    openssl
    zsh
  ];
in
{
  shell = 
    let
    
    # Environment variables that need to be loaded from a dotfile.
    dotenv = ''

    '';
    in
    pkgs.mkShell {
      name = "aiv-k8-dev";

      # The packages available in the development environment
      packages = cliTools;

      # Setup hook that prepares environment and config files
      shellHook = ''
        # Set up environment variables
        export PROJECT_ROOT=$PWD
        export DATA_DIR="$PROJECT_ROOT/.data"
        echo "Writing .env file..."
        cat > .env <<EOF
        ${dotenv}
        EOF
        set -a; source .envhost 2>/dev/null || true; set +a
        if [ -f .envhost ]; then
          set -a; source .envhost; set +a
          export GHCR_AUTH_STRING=$(echo -n "$GITHUB_USERNAME:$GHCR_PAT" | base64 -w 0)
          cat > "$DATA_DIR/ghcr-patch.yaml" << PATCH
          machine:
            registries:
              config:
                ghcr.io:
                  auth:
                    auth: "$GHCR_AUTH_STRING"
            time:
              bootTimeout: 2m
        PATCH
        fi

        export TALOS_VERSION="v1.11.0"
        export KUBECONFIG="$DATA_DIR/talos/kubeconfig"
        export TALOSCONFIG="$DATA_DIR/talos/talosconfig"
        export TALOS_STATE_DIR="$DATA_DIR/talos"
        export DIRENV_WARN_TIMEOUT=0
        export TF_DATA_DIR="$PROJECT_ROOT/.data/terraform"
        export TF_VAR_kubeconfig="$KUBECONFIG"
        export MC_CONFIG_DIR="$PROJECT_ROOT/.data/minio"
        export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ pkgs.openssl ]}:$LD_LIBRARY_PATH"
      '';
  };

  environment = {
    imports = [
      inputs.services-flake.processComposeModules.default
      (multiService ./tilt.nix)
      (multiService ./talos.nix)
      (multiService ./container_repository.nix)
    ];
    
    services = {
      container_repository = {
        docker = {
          enable = true;
          remoteUrl = "https://registry-1.docker.io";
          dataDir = ".data/repo/docker";
          localPort = 5000;
        };
        k8s = {
          enable = true;
          remoteUrl = "https://registry.k8s.io";
          dataDir = ".data/repo/k8s";
          localPort = 5001;
        };
        gcr = {
          enable = true;
          remoteUrl = "https://gcr.io";
          dataDir = ".data/repo/gcr";
          localPort = 5002;
        };
        ghcr = {
          enable = true;
          remoteUrl = "https://ghcr.io";
          dataDir = ".data/repo/ghcr";
          localPort = 5003;
        };
      };

      talos = {
        cluster = {
          enable = true;
          useSudo = true;
          dataDir = ".data/talos";
          controlplanes = 1;
          cpus = "4.0";
          memory = 8192;
          workers = 3;
          cpusWorkers = "4.0";
          memoryWorkers = 8192;
          disk = 5120;
          extra-disks = 2;
          extra-disks-size = 6144;
          provisioner = "qemu";
          registryMirrors = [
            "docker.io=http://10.5.0.1:5000"
            "registry.k8s.io=http://10.5.0.1:5001"
            "gcr.io=http://10.5.0.1:5002"
            "ghcr.io=http://10.5.0.1:5003"
          ];
          # This is defined in the .envrc
          #configPatches = ["./.data/ghcr-patch.yaml"];
        };
      };
      
      tilt = {
        tilt = {
          enable = true;
          dataDir = ".data/postgres";
          runtimeInputs = [
            pkgs.minio-client
          ];
          environment = {
            KUBECONFIG = ".data/talos/kubeconfig";
          };
        };
      };
    };
    
    settings.processes.cluster.depends_on = {
      docker.condition = "process_started";
      k8s.condition = "process_started";
      gcr.condition = "process_started";
      ghcr.condition = "process_started";
    };
    settings.processes.tilt.depends_on = {
      cluster.condition = "process_log_ready";
    };
  };
}