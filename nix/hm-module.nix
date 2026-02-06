{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.hyprstream;

  configText = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (k: v: "${k} = ${toString v}") cfg.settings
  );
in
{
  options.services.hyprstream = {
    enable = lib.mkEnableOption "hyprstream, virtual output manager for Hyprland streaming";

    package = lib.mkPackageOption pkgs "hyprstream" { };

    settings = lib.mkOption {
      type =
        with lib.types;
        attrsOf (oneOf [
          str
          int
          bool
        ]);
      default = { };
      example = lib.literalExpression ''
        {
           streaming_workspace = "9";
           virtual_resolution = "1920x1080@60";
           auto_enable = "true";
           on_streaming_enter = "notify-send 'Stream visible'";
           on_streaming_leave = "notify-send 'Stream hidden'";
         }
      '';
      description = "Configuration for hyprstream written to ~/.config/hyprstream/config";
    };

    systemdTarget = lib.mkOption {
      type = lib.types.str;
      default = "hyprland-session.target";
      description = "Systemd target to bind to.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."hyprstream/config" = lib.mkIf (cfg.settings != { }) {
      text = configText;
    };

    systemd.user.services.hyprstream = {
      Install = {
        WantedBy = [ cfg.systemdTarget ];
      };

      Unit = {
        ConditionEnvironment = "WAYLAND_DISPLAY";
        Description = "hyprstream - virtual output manager for Hyprland streaming";
        After = [ cfg.systemdTarget ];
        PartOf = [ cfg.systemdTarget ];
        X-Restart-Triggers = lib.mkIf (cfg.settings != { }) [
          (builtins.hashString "sha256" configText)
        ];
      };

      Service = {
        ExecStart = "${lib.getExe cfg.package} daemon";
        Restart = "always";
        RestartSec = "5";
      };
    };
  };
}
