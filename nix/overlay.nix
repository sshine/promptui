{ inputs, ... }:
{
  flake.overlays.default = final: prev: {
    promptui-core = inputs.self.packages.${final.stdenv.hostPlatform.system}.default;
  };
}
