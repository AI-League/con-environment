{ pkgs, lib, config, ... }: 
let
  cfg = config.nas;
  registries = {
    docker = { port = 5000; upstream = "https://registry-1.docker.io"; };
    k8s    = { port = 5001; upstream = "https://registry.k8s.io"; };
    gcr    = { port = 5002; upstream = "https://gcr.io"; };
    ghcr   = { port = 5003; upstream = "https://ghcr.io"; };
    quay   = { port = 5004; upstream = "https://quay.io"; };
  };
in {
  options.nas = with lib; {
    ip = mkOption {
      type = types.str;
      description = "Static IP address for the NAS";
    };
    gateway = mkOption {
      type = types.str;
      description = "Default gateway";
    };
    subnet = mkOption {
      type = types.str;
      description = "Subnet CIDR allowed to access NFS/Postgres (e.g. 10.10.10.0/24)";
    };
    interface = mkOption {
      type = types.str;
      default = "eth0";
      description = "Primary network interface name";
    };
  };

  # 2. Implementation
  config = {
    networking.hostName = "nixos-nas";
    networking.hostId = "8425e349"; 

    # Networking Configuration using Options
    networking.useDHCP = false;
    networking.interfaces.${cfg.interface}.ipv4.addresses = [{
      address = cfg.ip;
      prefixLength = 24;
    }];
    networking.defaultGateway = cfg.gateway;
    networking.nameservers = [ "1.1.1.1" ];

    # Firewall
    networking.firewall.allowedTCPPorts = [ 2049 5432 ] 
      ++ (lib.mapAttrsToList (n: v: v.port) registries);

    # NFS Server
    services.nfs.server.enable = true;
    services.nfs.server.exports = ''
      /mnt/pool/share ${cfg.subnet}(rw,sync,no_subtree_check)
    '';

    # PostgreSQL
    services.postgresql = {
      enable = true;
      package = pkgs.postgresql_16;
      dataDir = "/mnt/pool/postgres";
      enableTCPIP = true;
      authentication = pkgs.lib.mkOverride 10 ''
        local all all trust
        host  all all ${cfg.subnet} trust
      '';
    };

    # Storage
    boot.supportedFilesystems = [ "zfs" ];

    # Docker Registries
    virtualisation.oci-containers.backend = "docker";
    virtualisation.oci-containers.containers = lib.mapAttrs (name: reg: {
      image = "registry:2";
      ports = [ "${toString reg.port}:5000" ];
      environment = {
        REGISTRY_PROXY_REMOTEURL = reg.upstream;
        REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY = "/var/lib/registry";
      };
      volumes = [ "/mnt/pool/registries/${name}:/var/lib/registry" ];
    }) registries;

    # System Tools
    environment.systemPackages = with pkgs; [ zfs htop git ethtool hdparm smartmontools ];
    
    services.openssh.enable = true;
    users.users.root.password = "nixos";
    services.openssh.settings.PermitRootLogin = "yes";
  };
}