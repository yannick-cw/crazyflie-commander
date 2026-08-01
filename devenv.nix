{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  # to be used to share below
  db = {
    name = "missions";
    user = "postgres";
    pass = "password";
  };
  # this is a weird one -- not really static, gets injected somehow, picks first free port
  pgPort = toString config.processes.postgres.ports.main.value;
in
{
  languages.rust = {
    enable = true;
    channel = "stable";
    # simpler than in flake before
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
    # this is just the first port to look at - if it is taken, take +1
    # could be prevented with setting in devenv.yaml
    port = 5432;
    initialDatabases = [ db ];
  };

  # injects env variable in dev env
  env.DATABASE_URL = "postgres://${db.user}:${db.pass}@localhost:${pgPort}/${db.name}";

  packages = [
    pkgs.sqlx-cli
    pkgs.cargo-generate
    pkgs.cargo-expand
    pkgs.cargo-audit
    pkgs.pgcli
    pkgs.cargo-watch
  ];

  # creates a symlinked .env file - this is mostly just for rust rover
  files.".env".text = ''
    DATABASE_URL=${config.env.DATABASE_URL}
  '';

  # run with devenv tasks db:migrate - also auto runs when backend is started and when `devenv test` is run
  # @ready needs to be reached for postgres for this to run
  tasks."db:migrate" = {
    exec = "sqlx migrate run";
    after = [ "devenv:processes:postgres" ];
  };

  # needed for rust rover toolchain
  enterShell = ''
    mkdir -p "$PWD/.rust-rover"
    ln -sfn ${config.languages.rust.toolchainPackage} "$PWD/.rust-rover/toolchain"
  '';

  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };

  # waits for pg to start, also runs migration
  enterTest = ''
    wait_for_port ${pgPort}
    cargo test
  '';

  # this is not run for `devenv test`, and when run depends on postgres being ready first
  processes = lib.optionalAttrs (!config.devenv.isTesting) {
    backend = {
      exec = "cd mission-store && cargo watch -x run";
      after = [ "devenv:processes:postgres" ];
    };
  };
}
