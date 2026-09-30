{ pkgs, lib, ... }:

let
  version = "1.2.3";
  commonArgs = {
    name = "app-${version}";
    src = ./src;
  };
in
{
  inherit lib;
  package = pkgs.stdenv.mkDerivation (commonArgs // {
    buildInputs = [ pkgs.openssl pkgs.zlib ];
  });
  shell = pkgs.mkShell {
    packages = [ pkgs.pkg-config ];
  };
  imports = [ ./hardware.nix ../common/network.nix ];
  callPkg = pkgs.callPackage ./tools/foo.nix { };
  fromOther = import ../other/thing.nix;
}
