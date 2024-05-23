# Request Join Proxy

## What it does

This will contain a proxy that intentionally delivers outdated data.

It is useful for joining several requests going from ICP blockchain to
a site (so called HTTPS outcalls) into one request, thus eliminating
multiplying your API bill several times, and increasing API throttling
threshold.

## Running environment

Any modern OS capable of running Rust and optionally Redis.

## Compiling the app

[Install Rust](https://www.rust-lang.org/tools/install), if it is not installed.

Run:
```sh
cargo build --release
```

The executable `joining-proxy-rust[.exe]` appears in `target/release/` directory.

## Configuration

The config loads from `config.json` (remove `//` comments!) file in the current directory
or is specified by `-c config.json` flag:
```json
{
    "port": 8080,
    "our_secret": "re0agaejei0to2coothiv3Shu5anai0ree3aipuo", // simple Bearer authentication
    "upstream_prefix": "https://api.openai.com", // if specified, Host: header is ignored
    "cache_timeout": 3000, // how long to keep upstream responses in the cache
    "remove_request_headers": ["X-Not"], // remove these headers from requests to upstream
    "add_request_headers": [["Authorization", "Bearer <OPENAI_API_KEY>"]], // add for example an API key
    "remove_response_headers": ["X-Forwarded-For"], // remove these headers from upstream's response
    "add_response_headers": [["X-My", "<VALUE>"]], // add these headers to upstream's response
    "show_hit_miss": true, // add `X-JoinProxy-Response: Hit|Miss` header - cache hit or miss
    "upstream_connect_timeout": 10, 
    "upstream_read_timeout": 60,
    "add_forwarded_from_header": false, // add X-Forwarded-For: header the default false is fine, because why one needs this?
}
```