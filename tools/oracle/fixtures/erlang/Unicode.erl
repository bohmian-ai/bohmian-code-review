-module(unicode).

-export([héllo/1]).

héllo(Wörld) ->
    "üñïçödé " ++ Wörld.
