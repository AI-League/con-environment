{
  pkgs,
  output,
  kubelib,
}:
let
  ciliumValues = {
    ipam = {
      mode = "kubernetes";
    };

    kubeProxyReplacement = true;

    securityContext = {
      capabilities = {
        ciliumAgent = [
          "CHOWN"
          "KILL"
          "NET_ADMIN"
          "NET_RAW"
          "IPC_LOCK"
          "SYS_ADMIN"
          "SYS_RESOURCE"
          "DAC_OVERRIDE"
          "FOWNER"
          "SETGID"
          "SETUID"
        ];
        cleanCiliumState = [
          "NET_ADMIN"
          "SYS_ADMIN"
          "SYS_RESOURCE"
        ];
      };
    };

    cgroup = {
      autoMount.enabled = false;
      hostRoot = "/sys/fs/cgroup";
    };

    k8sServiceHost = "localhost";
    k8sServicePort = 7445;

    dnsproxy = {
      enabled = true;
    };

    hubble = {
      enabled = true;
      relay.enabled = true;
      ui.enabled = true;
    };

    localRedirectPolicy = true;

    bgpControlPlane = {
      enabled = true;
    };

    ingressController = {
      enabled = true;
      default = true;
      loadbalancerMode = "shared";
    };

    tunnelProtocol = "vxlan";

    cni = {
      chainingMode = "none";
      exclusive = true;
    };

    gatewayAPI = {
      enabled = true;
      enableAlpn = true;
      enableAppProtocol = true;
    };

    hostPort = {
      enabled = true;
    };

    nodePort = {
      enabled = true;
    };

    externalIPs = {
      enabled = true;
    };

    loadBalancer = {
      mode = "snat";
      serviceTopology = true;
    };
  };

  cilium_chart = kubelib.downloadHelmChart {
    repo = "https://helm.cilium.io/";
    chart = "cilium";
    version = "v1.18.3";
    chartHash = "sha256-f+3s8+EmXiiqJ5p4dUtpQHWGTYflrO6L9Nj1zMMgh6w=";
  };

  renderedCiliumManifests = kubelib.buildHelmChart {
    name = "cilium";
    chart = cilium_chart;
    namespace = "kube-system";
    values = ciliumValues; # <-- Use the new attrset here
    includeCRDs = true;
  };

in
pkgs.writeShellApplication {
  name = "con-generate-cilium-patch";
  runtimeInputs = with pkgs; [ coreutils gnused ];

  text = ''
    set -euo pipefail

    mkdir -p "$(dirname "${output}")"
    
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
      
    ) > "${output}" # Single redirection to the output file
    
    echo "✓ Cilium patch generated: ${output}" >&2
  '';
}