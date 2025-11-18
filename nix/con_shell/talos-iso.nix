{ pkgs }:
{
  # --- Required Arguments ---
  version, # The Talos version (e.g., "v1.7.0")
  sha256,  # The SHA256 hash of the file (see instructions below)

  # --- Optional Arguments ---
  artifactType ? "iso", # "iso", "installer", "kernel", "initramfs", "uki", "raw"
  platform ? "metal",   # "metal", "aws", "gcp", etc.
  arch ? "amd64",     # "amd64", "arm64"
  secureboot ? false, 
  schematic ? "376567988ad370138ad8b2698212367b8edcb69b5fd68c80be1f2ec7d603b4ba" # Default schematic
}:

let
  secbootSuffix = if secureboot then "-secureboot" else "";

  # Builds the file name based on the documentation's rules
  fileName =
    if artifactType == "iso" then "${platform}-${arch}${secbootSuffix}.iso"
    else if artifactType == "installer" then "${platform}-installer-${arch}${secbootSuffix}.tar"
    else if artifactType == "kernel" then "kernel-${arch}"
    else if artifactType == "initramfs" then "initramfs-${arch}.xz"
    else if artifactType == "uki" then "${platform}-${arch}${secbootSuffix}-uki.efi"
    else if artifactType == "raw" then "${platform}-${arch}${secbootSuffix}.raw.xz"
    else
      builtins.throw "Unsupported artifactType: ${artifactType}. Must be one of: iso, installer, kernel, initramfs, uki, raw";

  url = "https://factory.talos.dev/image/${schematic}/${version}/${fileName}";
in
pkgs.fetchurl {
  name = "talos-${version}-${fileName}";
  inherit url sha256;
}