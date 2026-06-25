{
  description = "air-gradient-rs: no_std firmware for an AirGradient Open Air (ESP32-C3)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # The toolchain (channel, target, components) is defined once in
        # rust-toolchain.toml and read here, so `nix develop`, `rustup`, and CI
        # all agree on exactly one toolchain.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # wokwi-cli isn't in nixpkgs, so we pull the upstream prebuilt binary.
        # We only target this dev box (aarch64-darwin); the macOS asset just
        # needs the exec bit. Bump `version` and `hash` together on update.
        wokwiCli = pkgs.stdenvNoCC.mkDerivation {
          pname = "wokwi-cli";
          version = "0.26.1";

          src = pkgs.fetchurl {
            url = "https://github.com/wokwi/wokwi-cli/releases/download/v0.26.1/wokwi-cli-macos-arm64";
            hash = "sha256-+WUSLcj7o9W/aLdu0BQ8e0zmCRAsLwgkPBEZibJAm8s=";
          };

          dontUnpack = true;
          dontStrip = true;

          installPhase = ''
            runHook preInstall
            install -Dm755 "$src" "$out/bin/wokwi-cli"
            runHook postInstall
          '';

          meta = {
            description = "Command-line interface for the Wokwi simulator";
            homepage = "https://github.com/wokwi/wokwi-cli";
            mainProgram = "wokwi-cli";
            platforms = [ "aarch64-darwin" ];
          };
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain

            # Flash + serial monitor over the C3's native USB-Serial-JTAG.
            pkgs.espflash
            # Full-flash backup/restore (the only artifact preserving NVS).
            pkgs.esptool

            # Snapshot-test review (`cargo insta review`).
            pkgs.cargo-insta
          ]
          # Emulator smoke test (`wokwi-cli`). Not in nixpkgs, so packaged from
          # the upstream prebuilt binary; only built for this dev box.
          ++ pkgs.lib.optional (system == "aarch64-darwin") wokwiCli;

          env = {
            # esp-println reads this for log filtering.
            ESP_LOG = "info";
          };

          shellHook = ''
            echo "air-gradient-rs dev shell"
            echo "  rust:     $(rustc --version)"
            echo "  espflash: $(espflash --version 2>/dev/null || echo 'not found')"
            echo ""
            echo "  host tests:  cargo test"
            echo "  firmware:    cargo build -p aq-indoor -p aq-outdoor --target riscv32imc-unknown-none-elf --release"
            echo "  flash:       (cd firmware/indoor && cargo run)   # or firmware/outdoor"
          '';
        };
      }
    );
}
