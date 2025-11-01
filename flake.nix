{
  description = "Workshop configuration for AIV";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    process-compose-flake = {
      url = "github:Platonic-Systems/process-compose-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    services-flake = {
      url = "github:juspay/services-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, fenix, process-compose-flake, services-flake, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      imports = [
        process-compose-flake.flakeModule
      ];

      perSystem = { config, self', pkgs, system, lib, ... }:
        let
          hostSystemName = if (builtins.getEnv "DEV_HOSTNAME") != "" then (builtins.getEnv "DEV_HOSTNAME") else "localhost";
          dev_shell = import ./nix/dev_shell.nix {
            inherit inputs pkgs system hostSystemName;
          };

          rustToolchain = fenix.packages.${system}.stable.minimalToolchain;

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          commonBuildInputs = with pkgs; [
            openssl
          ];
          
          commonNativeBuildInputs = with pkgs; [
            pkg-config
          ];

          binaries = {
            sidecar-bin = rustPlatform.buildRustPackage {
              pname = "sidecar";
              version = "0.1.0";
              src = ./crates;
              cargoLock.lockFile = ./Cargo.lock;

              buildInputs = commonBuildInputs;
              nativeBuildInputs = commonNativeBuildInputs;
              buildAndTestSubdir = "crates/sidecar";
              
              cargoBuildFlags = [ "-p" "sidecar" ];
              doCheck = true;

              meta = with lib; {
                mainProgram = "sidecar";
              };
            };

            hub-bin = rustPlatform.buildRustPackage {
              pname = "hub";
              version = "0.1.0";
              src = ./crates;
              cargoLock.lockFile = ./Cargo.lock;

              buildInputs = commonBuildInputs;
              nativeBuildInputs = commonNativeBuildInputs;
              buildAndTestSubdir = "crates/hub";
              
              cargoBuildFlags = [ "-p" "hub" ];
              doCheck = true;

              meta = with lib; {
                mainProgram = "hub";
              };
            };
          };

        in
        {
          process-compose."default" = dev_shell.environment;
          devShells.default = dev_shell.shell;
        };
    };
}