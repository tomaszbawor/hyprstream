{
  lib,
  rustPlatform,
  stdenv,
  version ? "git",
}:

rustPlatform.buildRustPackage {
  pname = "hyprstream";
  inherit version;
  src = ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  meta = {
    homepage = "https://github.com/tomasz/hyprstream";
    description = "Virtual output manager for Hyprland streaming privacy";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "hyprstream";
  };
}
