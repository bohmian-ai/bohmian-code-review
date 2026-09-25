{ lib, pkgs }:
let
  inner = { pkgs }:
    let
      helper = import ./helper.nix { inherit pkgs; };
    in
    {
      build = helper.build;
      callMe = pkgs.callPackage ./tools/{ pkg = "x"; };
    };
in
{
  result = inner { inherit pkgs; };
}
