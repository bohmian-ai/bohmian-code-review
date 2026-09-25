{ pkgs }:

let
  greeting = "héllo wörld 😀";
  build = pkgs.stdenv.mkDerivation {
    name = "café-1.0";
    buildInputs = [ pkgs.openssl ];
  };
in
{
  inherit greeting build;
}
