{ pkgs, ... }: {
  networking.hostName = "inspector";
  networking.useDHCP = true;

  # Enable SSH so you can connect
  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [ 
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIE/PhAuMI529/ah9/nY27UHo0G/UMCTsZcGhmYk+O3Lv admin@aivillage.org" 
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOugqVQLYj89EwYEGthEt0C7OlZh6xRelBdb3LvFDzJb sven@nbhd.ai" 
  ];

  # Diagnostic Tools
  environment.systemPackages = with pkgs; [
    # Storage
    parted
    gptfdisk
    util-linux       # lsblk, fdisk
    smartmontools    # smartctl

    # Network
    ethtool
    tcpdump
    conntrack-tools
    
    # Hardware / System
    pciutils         # lspci
    usbutils         # lsusb
    lshw             # Hardware lister
    dmidecode        # DMI table decoder
    htop
    neofetch
  ];
}s