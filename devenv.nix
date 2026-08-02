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
  pgPort = config.processes.postgres.ports.main.value;
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
    # so the user can create dbs and so..
    initialScript = ''
      ALTER ROLE "${db.user}" SUPERUSER;
    '';
    # this is just the first port to look at - if it is taken, take +1
    # could be prevented with setting in devenv.yaml
    port = 5432;
    initialDatabases = [ db ];
  };

  env = {
    # injects env variable in dev env, useful for sqlx to have compile time safety
    DATABASE_URL = "postgres://${db.user}:${db.pass}@localhost:${toString pgPort}/${db.name}";
    # prevents always needing running postgress when working, as database url above is present
    # and overwrites using /.sqlx cache
    SQLX_OFFLINE = "true";
  };

  packages = [
    pkgs.sqlx-cli
    pkgs.cargo-generate
    pkgs.cargo-expand
    pkgs.cargo-audit
    pkgs.pgcli
    pkgs.cargo-watch
  ];

  files = {
    # creates a symlinked .env file - this is mostly just for rust rover
    ".env".text = ''
      DATABASE_URL=${config.env.DATABASE_URL}
      SQLX_OFFLINE=true
    '';
    # creates a symlinked config, for rust rover
    "mission-store/configuration.yaml".yaml = {
      log_settings = {
        log_filter = "info,tower_http=debug,sqlx::query=debug";
        log_structured = false;
      };
      db = {
        user = "${db.user}";
        passwd = "${db.pass}";
        port = pgPort;
        url = "localhost";
        name = "${db.name}";
      };
    };
  };
  dotenv.disableHint = true; # I just write to it and dont read it here

  # run with devenv tasks db:migrate - also auto runs when backend is started and when `devenv test` is run
  # @ready needs to be reached for postgres for this to run
  tasks."db:migrate" = {
    exec = "sqlx migrate run";
    after = [ "devenv:processes:postgres" ];
  };

  # so we always have a fresh state - means no missions locally for now
  tasks."app:cleanup" = {
    exec = ''
      rm -rf .devenv/state/postgres
      rm -rf .devenv/test-state/postgres
    '';
    before = [ "devenv:processes:postgres" ];

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
    wait_for_port ${toString pgPort}
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
