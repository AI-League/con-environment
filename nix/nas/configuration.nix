{ config, pkgs, inputs, ... }:
  let
    inspectorBuild = inputs.self.nixosConfigurations.inspector.config.system.build;
    bootScript = pkgs.writeText "boot.ipxe" ''
      #!ipxe
      dhcp
      echo Starting Inspector Boot...
      kernel tftp://10.211.0.10/bzImage init=${inspectorBuild.toplevel}/init loglevel=4
      initrd tftp://10.211.0.10/initrd
      boot
    '';
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
      22   # Local ssh
      53   # DNS (dnsmasq)
      2049 # NFS
    ]; 
    allowedUDPPorts = [ 
      53   # DNS
      67   # DHCP
      69   # TFTP
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
  # 8. Main DHCP & DNS (Pure Dnsmasq PXE)
  # ==========================================

  services.resolved.enable = false;

  services.dnsmasq = {
    enable = true;
    alwaysKeepRunning = true; 
    
    settings = {
      interface = [ "enp1s0" ];
      bind-interfaces = true; 
      log-dhcp = true;

      # DNS
      domain-needed = true;
      bogus-priv = true;
      server = [ "1.1.1.1" "8.8.8.8" ];
      expand-hosts = true;
      domain = "cluster.local";

      # DHCP Subnet
      dhcp-range = [ "10.211.0.50,10.211.0.100,255.255.255.0,24h" ];

      # Options
      dhcp-option = [
        "option:router,10.211.0.1"
        "option:dns-server,10.211.0.10"
      ];

      # Static Hosts
      dhcp-host = [
        "aa:bb:cc:dd:ee:01,10.211.0.20,proxmox-node-01"
        "aa:bb:cc:dd:ee:02,10.211.0.21,k8s-control-plane"
        "aa:bb:cc:dd:ee:03,10.211.0.22,k8s-worker-01"
      ];
      address = [ "/nas/10.211.0.10" ];

      # ==========================================
      # TFTP & PXE Configuration
      # ==========================================
      enable-tftp = true;
      tftp-root = "/var/lib/tftpboot";

      # 1. Tagging: Detect if the client is BIOS (Legacy), UEFI, or iPXE
      dhcp-match = [
        "set:efi-x86_64,option:client-arch,7"
        "set:efi-x86_64,option:client-arch,9"
        "set:ipxe,175" # iPXE sends option 175
      ];

      # 2. Boot Logic (Chainloading)
      # If the client is NOT iPXE yet (!ipxe), send the iPXE bootloader.
      dhcp-boot = [
        "tag:!ipxe,tag:!efi-x86_64,undionly.kpxe"  # Legacy BIOS -> load undionly.kpxe
        "tag:!ipxe,tag:efi-x86_64,ipxe.efi"        # UEFI -> load ipxe.efi
        "tag:ipxe,boot.ipxe"                       # If iPXE is running -> load script
      ];
    };
  };

  # ==========================================
  # 9. PXE Files Setup (The "Plumbing")
  # ==========================================
  
  # This creates the TFTP directory and symlinks your build files into it
  # so Dnsmasq can serve them.
  systemd.tmpfiles.rules = [
    "d /var/lib/tftpboot 0755 root root -"
    "L+ /var/lib/tftpboot/ipxe.efi - - - - ${pkgs.ipxe}/ipxe.efi"
    "L+ /var/lib/tftpboot/undionly.kpxe - - - - ${pkgs.ipxe}/undionly.kpxe"
    "L+ /var/lib/tftpboot/bzImage - - - - ${inspectorBuild.kernel}/bzImage"
    "L+ /var/lib/tftpboot/initrd - - - - ${inspectorBuild.netbootRamdisk}/initrd"
    "L+ /var/lib/tftpboot/boot.ipxe - - - - ${bootScript}"
  ];
}