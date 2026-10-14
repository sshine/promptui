{ inputs, ... }:
{
  flake.overlays.default = final: _prev: {
    promptui-core = inputs.self.packages.${final.stdenv.hostPlatform.system}.default;
  };
}
