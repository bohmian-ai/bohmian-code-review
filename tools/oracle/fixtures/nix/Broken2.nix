{ pkgs }:

{
  goodAttr = pkgs.stdenv.mkDerivation {
    name = "good";
    src = ./src;
  };
}

let
  bad = if true then "a"
in
{ inherit bad; }
