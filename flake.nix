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
          ];

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
            echo "  firmware:    cargo build -p aq-firmware --target riscv32imc-unknown-none-elf --release"
            echo "  flash:       (cd firmware && cargo run)"
          '';
        };
      }
    );
}
