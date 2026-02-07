{
  description = "Virtual output manager for Hyprland streaming privacy";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default-linux";
  };

  outputs =
    {
      self,
      nixpkgs,
      systems,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      eachSystem = lib.genAttrs (import systems);

      mkDate =
        longDate:
        lib.concatStringsSep "-" [
          (builtins.substring 0 4 longDate)
          (builtins.substring 4 2 longDate)
          (builtins.substring 6 2 longDate)
        ];

      version = "0.1.0";

      pkgsFor = eachSystem (
        system:
        import nixpkgs {
          localSystem.system = system;
          overlays = [ self.overlays.default ];
        }
      );
    in
    {
      overlays.default = final: prev: {
        hyprstream = prev.callPackage ./nix/default.nix {
          version =
            version
            + "+date="
            + (mkDate (self.lastModifiedDate or "19700101"))
            + "_"
            + (self.shortRev or "dirty");
        };
      };

      packages = eachSystem (system: {
        default = self.packages.${system}.hyprstream;
        inherit (pkgsFor.${system}) hyprstream;
      });

      devShells = eachSystem (
        system:
        let
          pkgs = pkgsFor.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ pkgs.hyprstream ];

            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              gdb
              man-pages
              man-pages-posix
            ];

            shellHook = ''
              echo "hyprstream dev shell"
              echo "  cargo build        - build"
              echo "  cargo build --release - release build"
              echo "  cargo fmt          - format"
              echo "  cargo clippy       - lint"
              echo "  gdb target/debug/hyprstream - debug"
            '';
          };
        }
      );

      homeManagerModules.default = import ./nix/hm-module.nix;
      nixosModules.default = import ./nix/nixos-module.nix;

      formatter = eachSystem (system: pkgsFor.${system}.alejandra);
    };
}
