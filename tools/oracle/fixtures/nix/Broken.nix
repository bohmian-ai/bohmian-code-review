{ pkgs }:

let
  x = if true then "a"
in {
  inherit x;
  broken = [ 1 2
}
