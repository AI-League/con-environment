### **Phase 1: Build & Burn**

1. **Build the ISO**
Run this command in your flake directory. It will produce an ISO image in the `result/iso/` directory.
```bash
nix build .#nas-installer-iso

```


2. **Flash to USB**
Identify your USB stick (e.g., `/dev/sdX` on Linux or `/dev/diskN` on macOS) and flash the image.
*Replace `/dev/sdX` with your actual USB device ID.*
```bash
# Linux
sudo dd if=result/iso/nixos-*.iso of=/dev/sdX bs=4M status=progress conv=fsync

# macOS (use diskutil to unmount first)
sudo dd if=result/iso/nixos-*.iso of=/dev/rdiskN bs=4m status=progress

```

### **Phase 2: Boot & Connect**

3. **Boot the Beelink**
* Insert the USB into the Beelink NAS.
* Turn it on and enter BIOS (usually `Delete` or `F7`) to ensure it boots from USB.
* Wait ~1 minute for the system to initialize.


4. **SSH into the Installer**
Since we hardcoded the IP in `iso.nix`, you don't need to hunt for it.
```bash
# Replace with the static IP you set in iso.nix
ssh root@192.168.1.50

```

*(No password required; it uses the SSH key you baked in).*

### **Phase 3: Partitioning & Storage**

You need to prepare two things: the **OS Drive** (where NixOS lives) and the **ZFS Data Pool** (where your data lives).

5. **Prepare the OS Drive**
Identify your boot drive (usually the smaller NVMe/SSD, e.g., `/dev/nvme0n1`).
```bash
# 1. Create a Partition Table
parted /dev/nvme0n1 -- mklabel gpt

# 2. Create Boot Partition (512MB)
parted /dev/nvme0n1 -- mkpart ESP fat32 1MB 512MB
parted /dev/nvme0n1 -- set 1 esp on

# 3. Create Root Partition (Rest of disk)
parted /dev/nvme0n1 -- mkpart primary 512MB 100%

# 4. Format
mkfs.fat -F 32 -n BOOT /dev/nvme0n1p1
mkfs.ext4 -L nixos /dev/nvme0n1p2  # Or use ZFS for root if preferred

# 5. Mount Target
mount /dev/nvme0n1p2 /mnt
mkdir -p /mnt/boot
mount /dev/nvme0n1p1 /mnt/boot

```


6. **Create the ZFS Data Pool**
Your `configuration.nix` expects a ZFS dataset at `tank/share`. You must create this now, or the installed system will fail to boot.
```bash
# Identify your data drives (e.g., /dev/sda, /dev/sdb)
lsblk

# Create the pool (e.g., a mirror of two drives)
zpool create -f -m legacy tank mirror /dev/sda /dev/sdb

# Create the dataset
zfs create tank/share

```



---

### **Phase 4: Install & Configure**

7. **Generate Hardware Config**
NixOS needs to know about your specific hardware (disk UUIDs, kernel modules).
```bash
nixos-generate-config --root /mnt

```


8. **Apply Your Configuration**
We baked your custom config into the ISO at `/etc/nixos/configuration.nix`. We need to overwrite the default one generated in step 7.
```bash
# Copy your custom config to the mount target
cp /etc/nixos/configuration.nix /mnt/etc/nixos/configuration.nix

```


*Note: `hardware-configuration.nix` was generated in Step 7 and is already in place. Your `configuration.nix` imports it automatically.*
9. **Install NixOS**
```bash
nixos-install

```


10. **Finish**
```bash
reboot

```


Pull the USB stick. Your Beelink should reboot, claim the static IP configured in `configuration.nix`, and be ready for Tailscale authentication.