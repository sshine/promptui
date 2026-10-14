# Git hooks via hk-nix. The generated hk.pkl is symlinked into the repo root by
# the devshell startup hook (see devshell.nix); hk.pkl is gitignored.
{ inputs, ... }:
{
  imports = [ inputs.hk-nix.flakeModules.default ];
  perSystem =
    {
      config,
      pkgs,
      lib,
      ...
    }:
    let
      # Reference treefmt by absolute store path: the `nix flake check` hk-check
      # sandbox runs the pre-commit hook without the devshell PATH, so a bare
      # `treefmt` is not found there.
      treefmt = lib.getExe config.treefmt.build.wrapper;
    in
    {
      hk-nix.settings.hooks = {
        "pre-commit" = {
          fix = true;
          stash = "git";
          steps.treefmt = {
            check = "${treefmt} --fail-on-change --no-cache {{files}}";
            fix = "${treefmt} {{files}}";
          };
        };

        "pre-push".steps = {
          deadnix = {
            glob = "*.nix";
            check = "${lib.getExe pkgs.deadnix} --fail {{files}}";
          };
          clippy = {
            check = "cargo clippy --all-targets --all-features -- -D warnings";
          };
          lock-check = {
            check = "cargo metadata --locked --format-version 1 > /dev/null";
          };
        };

        # hk runs this one itself, so no tool needs pinning. Note it has no
        # equivalent of --require-scope; --allowed-types is the only policy knob.
        "commit-msg".steps.conventional.builtin = config.hk-nix.builtins.check_conventional_commit;
      };
    };
}
