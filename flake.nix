{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      system = "aarch64-darwin";
      # importing the nixpkgs path and calling the nixpkgs default function with the overlay
      # that adds rust
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      # override builds a new derivation of the rust package
      rustPackage = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
        ];
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [ rustPackage ];
        shellHook = ''
          mkdir -p "$HOME/.rust-rover"
          ln -sfn ${rustPackage} "$HOME/.rust-rover/toolchain"
        '';
      };
    };
}
