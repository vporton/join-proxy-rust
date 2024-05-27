# Joining Proxy

WARNING: It is not tested yet!

## What it does

This is a proxy that intentionally delivers outdated data (ignoring, for instance, `Cache-Control:` header).
It is intended mainly to direct IC outcalls to this proxy,
in order for do one, not 13 or more requests to an upstream in a single outcall.
Thus you, for example, pay 13x less for OpenAI tokens, if IC is connected to it through
this proxy.

It has special support for [Internet Computer](https://internetcomputer.org/), namely it can ask
IC to authenticate every request.

## Welcome

To get started, you might want to explore the project directory structure and the default configuration file..

If you want to start working on your project right away, you might want to try the following commands:

```bash
cd join_proxy/
cargo build --release
cargo run --release
```

## Configuration

The proxy app is called as:

```bash
joining-proxy [--config <TOML> | -c <TOML>]
```

where `<TOML>` is a [TOML](https://toml.io) file with configuration. By default the file `config.toml` from current directory is used.

An example of `config.toml`:

```toml
# The host and port to attach:
host = "localhost" # "localhost" by default
port = 8080 # 8080 by default

# Simple Bearer authentication. On IC platform you should use callback authentication instead.
# If you omit this entry, no Bearer authenticatio is done.
our_secret = "<KEY>"

upstream_prefix = "https://api.openai.com" # by default Host: header is used

ic_local = false # if to use a local testnet, DON'T SET THIS TO TRUE IN PRODUCTION
ic_url = "https://localhost:8000" # URL to connect to IC (for authorization), the default value is determined by `ic_local`

# If you omit this section, no authorization by callbacks is done.
# WARNING: In this case your proxy is eligible to unauthorized connections, such as stealing your OpenAI tokens.
[callback]
canister = "a3shf-5eaaa-aaaaa-qaafa-cai" # the principal of the canister used for authorization
func = "checkRequest" # the shared method used for authorization

[cache]
cache_timeout = "1m" # How long responses are cached. See for format: https://chatgpt.com/share/4fb8f7c9-ad48-4cfe-a875-efcb6d36bbf1

# Timeouts for a connection from the proxy to an upstream.
[upstream_timeouts]
connect_timeout = "20s" # how quickly an upstream answers
read_timeout = "30s" # single socket read operation timeout
total_timeout = "60s" # total timeout from request start to request end

# Modify headers in requests to an upstream.
[request_headers]
remove = ["X-Proxy"] # remove these headers 
add = [["Authorization": "Bearer <OPENAI-API-KEY>"]] # add headers

# Modify headers in responses (e.g. to IC) from our proxy.
[response_headers]
remove = ["Cookie"] # remove these headers
add = [["Cookie", "userId=789"]] # add these headers
show_hit_miss = false # true by default. Add `X-JoinProxy-Response: [Hit | Miss]` header
add_forwarded_from_header = false # Add `X-Forwarded-From` useless but widespread HTTP header to the response
```

## IC Code

For examples of IC code compatible with this proxy, see `motoko/example/` directory.