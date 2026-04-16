{ inputs, ... }:
{
  perSystem =
    { system, ... }:
    let
      cargoNix = inputs.crate2nix.tools.${system}.appliedCargoNix {
        name = "promptui";
        src = ../.;
      };
    in
    {
      checks = {
        promptui-core = cargoNix.workspaceMembers.promptui-core.build;
      };

      packages = {
        default = cargoNix.workspaceMembers.promptui-core.build;
      };
    };
}
