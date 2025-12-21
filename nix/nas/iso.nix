{ config, pkgs, ... }:

{
  packages.x86_64-linux.installer-iso = nixos-generators.nixosGenerate {
    system = "x86_64-linux";
    format = "install-iso";
    modules = [
      ({ pkgs, ... }: {
        services.openssh.enable = true;
        users.users.root.openssh.authorizedKeys.keys = [ "ssh-ed25519 AAA..." ];

        # --- ZFS SUPPORT START ---
        # 1. Load ZFS kernel modules
        boot.supportedFilesystems = [ "zfs" ];
        
        # 2. ZFS requires a hostId (just 8 random hex digits)
        networking.hostId = "8425e349"; 
        
        # 3. Add helpful tools for partitioning and ZFS management
        environment.systemPackages = with pkgs; [ 
          parted      # For the OS drive
          gptfdisk    # For the NVMe drives
          zfs         # Explicitly include ZFS tools
          git
          neovim 
        ];
        # --- ZFS SUPPORT END ---
      })
    ];
  };
}