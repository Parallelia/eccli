# eccli

Friendly command-line client for the [Parallelia `ec`](https://github.com/Parallelia/ec)
Electoral Commission daemon. Written in Rust, 100% compatible with the current `ec`
gRPC Admin API.

> Status: under active development — see the [roadmap (issue #1)](https://github.com/Parallelia/eccli/issues/1).

## Build

```sh
cargo build --release
# binary at target/release/eccli
```

Requires `protoc` (`apt install protobuf-compiler`).

## Global options

- `--server <URL>` — ec gRPC endpoint (default `http://127.0.0.1:50051`, env `EC_SERVER`).
- `--token <TOKEN>` — admin bearer token, only needed when the ec sets `EC_ADMIN_TOKEN`
  (env `EC_ADMIN_TOKEN`).

## Commands

- `check` — verify connectivity to the ec daemon.

Election, candidate and registration-token commands are landing incrementally;
see issue #1 for the full plan.
