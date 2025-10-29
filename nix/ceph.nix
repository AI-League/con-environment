{ config, lib, name, pkgs, ... }:

let
  inherit (lib) types mkOption;

  rookCephChart = pkgs.fetchurl {
    url = "https://charts.rook.io/release/rook-ceph-v1.15.8.tgz";
    sha256 = "sha256-R+xLp0u4h5KWHVnOqR12N1vgEgRZMzzHm1q8sWMF164=";
  };
  rookCephClusterChart = pkgs.fetchurl {
    url = "https://charts.rook.io/release/rook-ceph-cluster-v1.15.8.tgz";
    sha256 = "sha256-7wYL0te8jKQKmbNbBuO+a0mx7re940bj0ctPl1Exp+s="; 
  };

  setupScript = pkgs.writeShellApplication {
    name = "install-rook-ceph";
    
    runtimeInputs = [
      config.kubectlPackage
      config.helmPackage
    ];

    text = ''
      set -euo pipefail

      echo "Installing Rook Ceph operator..."
      helm upgrade --install \
        --create-namespace \
        --namespace rook-ceph \
        --wait \
        rook-ceph ${(lib.escapeShellArg rookCephChart)}

      echo "Labeling namespace for pod security..."
      kubectl label namespace rook-ceph pod-security.kubernetes.io/enforce=privileged --overwrite

      echo "Installing Rook Ceph cluster..."
      helm upgrade --install \
        --namespace rook-ceph \
        --set operatorNamespace=rook-ceph \
        --wait \
        rook-ceph-cluster ${(lib.escapeShellArg rookCephClusterChart)}

      echo "Applying custom configurations..."
      kubectl apply -f ${(lib.escapeShellArg config.configDir)}/

      echo "Waiting for Ceph cluster to be ready..."
      until kubectl get cephcluster -n rook-ceph rook-ceph 2>&1; do
        sleep 5
      done
      
      TIMEOUT=600
      ELAPSED=0
      while [ $ELAPSED -lt $TIMEOUT ]; do
        HEALTH=$(kubectl -n rook-ceph get cephcluster rook-ceph -o jsonpath='{.status.ceph.health}' 2>&1 || echo "UNKNOWN")
        echo "Ceph cluster health: $HEALTH ($ELAPSED/$TIMEOUT seconds)"
        if [ "$HEALTH" = "HEALTH_OK" ] || [ "$HEALTH" = "HEALTH_WARN" ]; then
          break
        fi
        sleep 10
        ELAPSED=$((ELAPSED + 10))
      done
      
      echo "Rook Ceph cluster ready"
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
    kubectlPackage = mkOption {
      type = types.package;
      default = pkgs.kubectl;
      description = "The kubectl package to use.";
    };
    kubeconfig = mkOption {
      type = types.str;
      description = "Path to the kubeconfig file for the service.";
    };
    configDir = mkOption {
      type = types.path;
      description = "Directory containing additional Rook Ceph configurations.";
    };
  };

  config = {
    outputs.settings.processes."${name}" = {
      command = "${setupScript}/bin/install-rook-ceph";
      environment = {
        KUBECONFIG = config.kubeconfig;
      };
      ready_log_line = "Rook Ceph cluster ready";
    };
  };
}