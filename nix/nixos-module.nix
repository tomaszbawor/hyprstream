{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.hyprstream;
in
{
  options.programs.hyprstream = {
    enable = lib.mkEnableOption "hyprstream, virtual output manager for Hyprland streaming";

    package = lib.mkPackageOption pkgs "hyprstream" { };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
  };
}
