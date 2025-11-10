# =============================================================================
# File: nix/con_shell.nix
# Physical Talos Cluster Configuration Module
# =============================================================================
{ pkgs, lib, config, name, ... }:
let
  inherit (lib) types mkOption mkIf;

  # Import submodules
  patchGenerators = import ./con_shell/patch_generators.nix { 
    inherit pkgs lib; 
    conConfig = config;
    ciliumValuesFile = config.ciliumValuesFile;
  };
  
  configGenerators = import ./con_shell/config_generators.nix { 
    inherit pkgs lib; 
    conConfig = config; 
  };
  
  clusterOps = import ./con_shell/cluster_ops.nix { 
    inherit pkgs lib; 
    conConfig = config; 
  };

  # Main CLI that routes commands
  conShellCli = pkgs.writeShellApplication {
    name = "con-shell";
    runtimeInputs = with pkgs; [ 
      talosctl 
      kubectl 
      kubernetes-helm 
      coreutils 
      gnused 
      gnugrep 
      gawk
    ];
    
    text = ''
      set -euo pipefail
      
      # Color output
      RED='\033[0;31m'
      GREEN='\033[0;32m'
      YELLOW='\033[1;33m'
      BLUE='\033[0;34m'
      NC='\033[0m'
      
      info() { echo -e "''${BLUE}ℹ''${NC} $1"; }
      success() { echo -e "''${GREEN}✓''${NC} $1"; }
      warn() { echo -e "''${YELLOW}⚠''${NC} $1"; }
      error() { echo -e "''${RED}✗''${NC} $1"; exit 1; }
      step() { echo -e "''${YELLOW}▶''${NC} $1"; }
      
      # Configuration
      export CONFIG_DIR="''${CONFIG_DIR:-.con}"
      export PATCHES_DIR="''${CONFIG_DIR}/patches"
      export CONFIGS_DIR="''${CONFIG_DIR}/configs"
      
      cmd_help() {
        cat << 'EOF'
      con-shell - Physical Talos Cluster Configuration Generator
      
      USAGE:
          con-shell <command> [args]
      
      COMMANDS:
          init                    Initialize configuration directory
          generate-patches        Generate all patch files
          generate-configs        Generate machine configs (requires patches)
          apply [node-ip]         Apply configs to node(s)
          bootstrap               Bootstrap the cluster
          health                  Check cluster health
          help                    Show this help message
      
      WORKFLOW:
          1. con-shell init
          2. Edit .con/cluster.conf with your settings
          3. con-shell generate-patches
          4. Review/edit patches in .con/patches/
          5. con-shell generate-configs
          6. Boot your nodes with Talos ISO
          7. con-shell apply
          8. Wait 2-3 minutes for installation
          9. con-shell bootstrap
          10. con-shell health
      
      CONFIGURATION:
          Edit .con/cluster.conf or set environment variables:
          
          CLUSTER_NAME            - Cluster name
          CLUSTER_ENDPOINT        - Kubernetes API endpoint
          VIP_IP                  - Virtual IP for HA
          GATEWAY                 - Network gateway
          NETWORK_CIDR            - Node network CIDR
          TALOS_VERSION           - Talos version
          CILIUM_VERSION          - Cilium version
          INSTALL_DISK            - Installation disk
          
          CONTROL_PLANE_IPS       - Array of control plane IPs
          WORKER_IPS              - Array of worker IPs
      
      EXAMPLES:
          # Full workflow
          con-shell init
          con-shell generate-patches
          con-shell generate-configs
          con-shell apply
          con-shell bootstrap
          
          # Apply to specific node
          con-shell apply 10.10.10.21
      EOF
      }
      
      cmd="''${1:-help}"
      shift || true
      
      case "$cmd" in
        init)
          ${lib.getExe patchGenerators.initScript}
          ;;
        generate-patches)
          ${lib.getExe patchGenerators.generateAllScript}
          ;;
        generate-configs)
          ${lib.getExe configGenerators.generateConfigsScript}
          ;;
        apply)
          ${lib.getExe clusterOps.applyConfigScript} "$@"
          ;;
        bootstrap)
          ${lib.getExe clusterOps.bootstrapScript}
          ;;
        health)
          ${lib.getExe clusterOps.healthCheckScript}
          ;;
        help|--help|-h)
          cmd_help
          ;;
        *)
          error "Unknown command: $cmd (try 'help')"
          ;;
      esac
    '';
  };

in
{
  options = {
    clusterName = mkOption {
      type = types.str;
      default = "talos-physical";
      description = "Name of the physical cluster";
    };

    clusterEndpoint = mkOption {
      type = types.str;
      default = "https://10.10.10.11:6443";
      description = "Kubernetes API endpoint";
    };

    vipAddress = mkOption {
      type = types.nullOr types.str;
      default = "10.10.10.11";
      description = "Virtual IP address for HA control plane";
    };

    controlPlaneIPs = mkOption {
      type = types.listOf types.str;
      default = [ "10.10.10.21" ];
      description = "List of control plane node IPs";
    };

    workerIPs = mkOption {
      type = types.listOf types.str;
      default = [ "10.10.10.22" "10.10.10.23" "10.10.10.24" ];
      description = "List of worker node IPs";
    };

    gateway = mkOption {
      type = types.str;
      default = "10.10.10.1";
      description = "Network gateway";
    };

    networkCIDR = mkOption {
      type = types.str;
      default = "10.10.10.0/24";
      description = "Network CIDR for node subnet";
    };

    talosVersion = mkOption {
      type = types.str;
      default = "v1.11.0";
      description = "Talos Linux version";
    };

    ciliumVersion = mkOption {
      type = types.str;
      default = "1.16.5";
      description = "Cilium CNI version";
    };

    ciliumValuesFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "Path to custom Cilium values.yaml file. If null, generates default values.";
      example = "./setup/k8/cilium-values.yaml";
    };

    installDisk = mkOption {
      type = types.str;
      default = "/dev/sda";
      description = "Default installation disk";
    };

    configDir = mkOption {
      type = types.str;
      default = ".con";
      description = "Directory for configuration files";
    };
  };

  config = mkIf config.enable {
    outputs.devShells = {
      "${name}" = pkgs.mkShell {
        name = "con-shell-environment";
        
        packages = with pkgs; [
          talosctl
          kubectl
          kubernetes-helm
          conShellCli
        ];

        shellHook = ''
          echo "🔧 Physical Cluster Configuration Environment"
          echo ""
          echo "Available commands:"
          echo "  con-shell init              - Initialize configuration"
          echo "  con-shell generate-patches  - Generate Talos patches"
          echo "  con-shell generate-configs  - Generate node configs"
          echo "  con-shell apply            - Apply configurations"
          echo "  con-shell bootstrap        - Bootstrap cluster"
          echo "  con-shell health           - Check cluster health"
          echo ""
          echo "Configuration: ${config.configDir}"
          echo "Cluster: ${config.clusterName}"
          echo "Endpoint: ${config.clusterEndpoint}"
          echo "Control Planes: ${toString (builtins.length config.controlPlaneIPs)}"
          echo "Workers: ${toString (builtins.length config.workerIPs)}"
          echo ""
        '';
      };
    };
  };
}