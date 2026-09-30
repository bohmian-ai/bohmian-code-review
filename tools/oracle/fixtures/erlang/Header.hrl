-define(MAX_RETRIES, 3).
-define(DEFAULT_HOST, "localhost").
-define(CALL(M), gen_server:call(?MODULE, M)).

-record(config, {host, port = 8080, retries = ?MAX_RETRIES}).
