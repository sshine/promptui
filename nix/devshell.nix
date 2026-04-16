{ self, inputs, ... }:
{
  imports = [ inputs.devshell.flakeModule ];
  perSystem =
    {
      config,
      system,
      pkgs,
      lib,
      ...
    }:
    let
      rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
      lefthook-check = inputs.lefthook-nix.lib.${system}.run {
        src = self;
        config = {
          pre-commit.commands.treefmt = {
            run = "treefmt --fail-on-change --no-cache {staged_files}";
          };
        };
      };
    in
    {
      checks.lefthook-check = lefthook-check;
      devshells.default = {
        packages = [
          rust-toolchain
          config.treefmt.build.wrapper
          pkgs.cargo-watch
          pkgs.cargo-insta
          pkgs.stdenv.cc
          pkgs.just
        ];

        env = [
          {
            name = "RUST_BACKTRACE";
            value = "1";
          }
          {
            name = "LEFTHOOK_BIN";
            value = toString (
              pkgs.writeShellScript "lefthook-dumb-term" ''
                exec env TERM=dumb ${lib.getExe pkgs.lefthook} "$@"
              ''
            );
          }
        ];

        devshell.motd = "";
        devshell.startup.lefthook.text = lefthook-check.shellHook;
      };
    };
}
