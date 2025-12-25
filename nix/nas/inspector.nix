{ pkgs, inputs, lib, ... }: {  # Added 'inputs' and 'lib' to arguments
  networking.hostName = "inspector";

  # Enable SSH
  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [ 
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIE/PhAuMI529/ah9/nY27UHo0G/UMCTsZcGhmYk+O3Lv admin@aivillage.org" 
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOugqVQLYj89EwYEGthEt0C7OlZh6xRelBdb3LvFDzJb sven@nbhd.ai" 
  ];

  # 1. Install your Custom Inspector Package
  environment.systemPackages = with pkgs; [
    inputs.self.packages.${pkgs.system}.inspector-bin # Your Rust tool
    
    # Existing tools...
    parted
    gptfdisk
    util-linux
    smartmontools
    ethtool
    tcpdump
    conntrack-tools
    pciutils
    usbutils
    lshw
    dmidecode
    htop
    neofetch
  ];

  # 2. Mount the NAS (NFS)
  # The NAS IP is 10.211.0.10 and export is /mnt/data (from nix/nas/configuration.nix)
  fileSystems."/mnt/nas" = {
    device = "10.211.0.10:/mnt/data";
    fsType = "nfs";
    options = [ "rw" "soft" "retry=5" "nolock" ]; # 'soft' avoids hanging if NAS is unreachable
  };

  # 3. Systemd Service to Run Inspector and Save Report
  systemd.services.inspector-report = {
    description = "Run Hardware Inspector and save to NAS";
    
    # Run after network and mount are ready
    after = [ "network.target" "mnt-nas.mount" ];
    requires = [ "mnt-nas.mount" ];
    
    # Run automatically on boot
    wantedBy = [ "multi-user.target" ];
    
    serviceConfig = {
      Type = "oneshot";
      # Ensure the output directory exists (optional, but safe)
      # Run the tool and redirect stdout to a YAML file on the share
      Script = ''
        ${inputs.self.packages.${pkgs.system}.inspector-bin}/bin/inspector inspect > /mnt/nas/inspector-report-$(hostname).yaml
      '';
    };
  };

  system.stateVersion = "24.11"; 
}