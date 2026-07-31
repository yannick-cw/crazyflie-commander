{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  dotenv.enable = true;
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rust-src"
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };
  services.postgres = {
    enable = true;
    listen_addresses = "localhost";
    port = 5432;
    initialDatabases = [ { name = "missions"; } ];
  };
  packages = [
    pkgs.sqlx-cli
    pkgs.cargo-generate
    pkgs.cargo-expand
    pkgs.cargo-audit
    pkgs.cargo-watch
  ];

  enterShell = ''
    mkdir -p "$PWD/.rust-rover"
    ln -sfn ${config.languages.rust.toolchainPackage} "$PWD/.rust-rover/toolchain"
  '';

  #git-hooks.hooks = {
  #rustfmt.enable = true;
  #clippy.enable = true;
  #};
  enterTest = ''
    wait_for_port 5432
    cargo test
  '';

  processes = lib.optionalAttrs (!config.devenv.isTesting) {
    backend = {
      exec = "cd mission-store && cargo watch -x run";
      after = [ "devenv:processes:postgres" ];
    };
  };
}
