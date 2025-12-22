{ pkgs }:
{
  # --- Required Arguments ---
  version, # The Talos version (e.g., "v1.7.0")
  sha256,  # The SHA256 hash of the ISO file (you must update this after changing extensions)

  # --- Optional Arguments ---
  artifactType ? "iso", # "iso", "installer", "kernel", "initramfs", "uki", "raw"
  platform ? "metal",   # "metal", "aws", "gcp", etc.
  arch ? "amd64",       # "amd64", "arm64"
  secureboot ? false,
  
  # --- Customization Arguments ---
  # If 'schematic' is provided explicitly, it takes precedence.
  schematic ? null,
  
  # List of official system extensions (e.g., ["siderolabs/gvisor", "siderolabs/amd-ucode"])
  systemExtensions ? [], 
  
  # List of extra kernel arguments
  extraKernelArgs ? [],
  
  # Meta configuration (e.g. initial Talos META)
  meta ? {}
}:

let
  # The default "vanilla" schematic ID (used if no customizations are provided)
  defaultSchematicId = "376567988ad370138ad8b2698212367b8edcb69b5fd68c80be1f2ec7d603b4ba";

  # Construct the schematic configuration object
  schematicConfig = {
    customization = {
      systemExtensions = {
        officialExtensions = systemExtensions;
      };
      extraKernelArgs = extraKernelArgs;
      meta = meta;
    };
  };

  # Serialize to JSON. Talos Image Factory accepts JSON as the schematic body.
  # We use JSON here because it ensures a deterministic string for hashing.
  schematicContent = builtins.toJSON schematicConfig;

  # Determine the schematic ID
  # 1. Use explicitly provided ID if present.
  # 2. If no customizations are specified, use the default vanilla ID.
  # 3. Otherwise, calculate the SHA256 hash of the content (Talos Image Factory uses content-addressing).
  computedSchematicId = 
    if schematic != null then schematic
    else if (systemExtensions == [] && extraKernelArgs == [] && meta == {}) then defaultSchematicId
    else builtins.hashString "sha256" schematicContent;

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

  url = "https://factory.talos.dev/image/${computedSchematicId}/${version}/${fileName}";

in
pkgs.fetchurl {
  name = "talos-${version}-${fileName}";
  inherit url sha256;

  # Pass through the schematic info and a helper script to register it.
  # Use this script if the build fails with a 404 (meaning the factory hasn't seen this config yet).
  passthru = {
    inherit schematicContent;
    schematicId = computedSchematicId;
    
    registerScript = pkgs.writeShellScript "register-schematic" ''
      echo "--- Registering Talos Schematic ---"
      echo "ID:      ${computedSchematicId}"
      echo "Content: ${schematicContent}"
      echo ""
      ${pkgs.curl}/bin/curl -f -X POST --data-binary '${schematicContent}' https://factory.talos.dev/schematics
      echo ""
      echo "-----------------------------------"
      echo "Success! The factory now recognizes this schematic."
    '';
  };
}