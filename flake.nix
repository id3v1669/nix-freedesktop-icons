{
  description = "Crosshair";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default-linux";
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    systems,
    ...
  }: let
    eachSystem = nixpkgs.lib.genAttrs (import systems);

    pkgsFor = system:
      import nixpkgs {
        inherit system;
        overlays = [];
      };
  in {
    devShells = eachSystem (system: let pkgs = pkgsFor system; in {
      default = pkgs.mkShell {
        name = "nix-freedesktop-icons-dev-shell";
        nativeBuildInputs = with pkgs; [
          # Compilers
          cargo
          rustc
          scdoc

          #libs
          pkg-config
          glib
          gtk4
          gdk-pixbuf
          cairo
          pango
          gnuplot
          #to pass tests
          adwaita-icon-theme
          arc-icon-theme

          # Tools
          cargo-audit
          cargo-deny
          clippy
          rust-analyzer
          rustfmt
        ];
      };
    });

    formatter.x86_64-linux = inputs.nixpkgs.legacyPackages.x86_64-linux.alejandra;
  };
}
