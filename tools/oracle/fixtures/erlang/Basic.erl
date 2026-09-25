%% Basic module exercising the Erlang extractor.
-module(basic).

-export([start/0, ping/1]).

-include("basic.hrl").
-include_lib("kernel/include/file.hrl").

-define(TIMEOUT, 5000).
-define(LOG(E), logger:info(E)).

-record(user, {id, name, age}).

-type user() :: #user{}.
-opaque token() :: binary().

-spec start() -> ok.
start() ->
    ?LOG("starting"),
    spawn(fun worker/0),
    gen_server:call(?SERVER, ping),
    ok.

-spec ping(user()) -> ok.
ping(#user{id = Id}) ->
    dispatch(Id).

worker() ->
    timer:sleep(?TIMEOUT),
    ok.

dispatch(Id) ->
    case Id of
        1 -> ok;
        _ -> error
    end.

f() ->
    {user, 1, "bob", 30}.
