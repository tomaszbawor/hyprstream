{
  lib,
  stdenv,
  version ? "git",
}:

stdenv.mkDerivation {
  pname = "hyprstream";
  inherit version;
  src = ../.;

  makeFlags = [
    "PREFIX=$(out)"
  ];

  meta = {
    homepage = "https://github.com/tomasz/hyprstream";
    description = "Virtual output manager for Hyprland streaming privacy";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "hyprstream";
  };
}
