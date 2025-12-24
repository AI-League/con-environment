# hosts/my-server/configuration.nix
{ config, pkgs, inputs, ... }:
  let
    # Ensure 'specialArgs = { inherit inputs; };' is set in your flake.nix
    inspectorBuild = inputs.self.nixosConfigurations.inspector.config.system.build;
  in
{
  imports =
    [
      ./hardware-configuration.nix
    ];

  # ==========================================
  # 1. Boot & System Basics
  # ==========================================
  
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;

  networking.hostName = "cluster-control";
  networking.hostId = "8425e349"; 

  # ==========================================
  # 2. ZFS & Storage Configuration
  # ==========================================
  
  boot.supportedFilesystems = [ "zfs" ];
  services.zfs.autoScrub.enable = true;

  fileSystems."/mnt/data" = {
    device = "tank/share";
    fsType = "zfs";
    options = [ "zfsutil" ]; 
  };

  # ==========================================
  # 3. Networking & Firewall
  # ==========================================
  
  networking.interfaces.enp1s0.ipv4.addresses = [{
    address = "10.211.0.10";
    prefixLength = 24;
  }];
  networking.defaultGateway = "10.211.0.1";
  networking.nameservers = [ "1.1.1.1" "8.8.8.8" ];

  services.tailscale = {
    enable = true;
    authKeyFile = "/var/keys/tailscale_key";
    extraUpFlags = [ "--ssh" ];
  };

  networking.firewall = {
    enable = true;
    trustedInterfaces = [ "tailscale0" ];
    
    # FIX: You must open ports for services running on the Physical LAN (enp1s0)
    allowedTCPPorts = [ 
        53   # DNS (dnsmasq)
        2049 # NFS
    ]; 
    allowedUDPPorts = [ 
        53   # DNS (dnsmasq)
        67   # DHCP (dnsmasq)
             # Note: Pixiecore handles its own ports (69/4011) via the service module
    ];
  };

  # ==========================================
  # 4. SSH Configuration
  # ==========================================
  
  services.openssh = {
    enable = true;
    openFirewall = false; # Keeps SSH closed on LAN (Tailscale only)
    settings = {
      PasswordAuthentication = false;
      PermitRootLogin = "no";
    };
  };

  # ==========================================
  # 5. NFS Server Configuration
  # ==========================================
  
  services.nfs.server = {
    enable = true;
    # FIX: Updated subnet to match your actual network (10.211.0.0/24)
    exports = ''
      /mnt/data 10.211.0.0/24(rw,nohide,insecure,no_subtree_check,no_root_squash)
    '';
  };

  # ==========================================
  # 6. User Account
  # ==========================================

  security.sudo.wheelNeedsPassword = false;

  users.users.admin = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOugqVQLYj89EwYEGthEt0C7OlZh6xRelBdb3LvFDzJb sven@nbhd.ai" 
    ];
  };

  # ==========================================
  # 7. Utilities
  # ==========================================
  environment.systemPackages = with pkgs; [
    vim
    nano
    talosctl
    kubectl
    git
    htop
    zfs
    zsh
    k9s
    cilium-cli
    hubble
    nmap
    tcpdump
  ];

  # ==========================================
  # 8. Main DHCP & DNS
  # ==========================================

  services.resolved.enable = false;

  services.dnsmasq = {
    enable = true;
    alwaysKeepRunning = true; 
    
    settings = {
      interface = [ "enp1s0" ];
      bind-interfaces = true; 

      # DNS
      domain-needed = true;
      bogus-priv = true;
      server = [ "1.1.1.1" "8.8.8.8" ];
      expand-hosts = true;
      domain = "cluster.local";

      # DHCP Subnet: Physical LAN (10.211.0.0/24)
      # Pool: .50 - .100 (VMs)
      # Reserved: .101 - .200 (Hidden from DHCP for Cilium)
      dhcp-range = [
        "10.211.0.50,10.211.0.100,255.255.255.0,24h"
      ];

      # Options
      dhcp-option = [
        "option:router,10.211.0.1"    # Gateway is Unifi
        "option:dns-server,10.211.0.10"       # DNS is THIS Server
      ];

      # Static Hosts (Physical Infrastructure)
      dhcp-host = [
        "aa:bb:cc:dd:ee:01,10.211.0.20,proxmox-node-01"
        "aa:bb:cc:dd:ee:02,10.211.0.21,k8s-control-plane"
        "aa:bb:cc:dd:ee:03,10.211.0.22,k8s-worker-01"
      ];

      # Hostname Alias
      address = [ "/nas/10.211.0.10" ];

      # PXE Boot configuration
      dhcp-boot = [
        "pixiecore.0,cluster-control,10.211.0.10"
      ];
    };
  };

  # ==========================================
  # 9. PXE / Netboot Server (Inspector)
  # ==========================================

  services.pixiecore = {
    enable = true;
    openFirewall = true; # Automatically opens 69/UDP and 4011/UDP
    dhcpNoBind = true;   # CRITICAL: Allows dnsmasq to handle port 67
    mode = "boot"; 
    
    kernel = "${inspectorBuild.kernel}/bzImage";
    initrd = "${inspectorBuild.netbootRamdisk}/initrd";
    cmdLine = "init=${inspectorBuild.toplevel}/init loglevel=4";
  };

  system.stateVersion = "24.11"; 
}