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
- `--json` — emit machine-readable JSON instead of human-readable output.
- `-y`, `--yes` — skip confirmation prompts for destructive commands.

## Commands

### Connectivity

- `check` — verify connectivity to the ec daemon.

### Elections

- `create-election --name <NAME> --start-time <TS> (--duration <SECS> | --end-time <TS>)`
  `[--rules-id <ID>] [--candidates-file <PATH> | --candidates-json <JSON>]`
- `get-election --election-id <ID>`
- `list-elections`
- `cancel-election --election-id <ID>` — destructive, prompts unless `--yes`.

Times accept either a relative offset in seconds (values below 1,000,000,000) or an
absolute unix timestamp. `--duration` and `--end-time` are mutually exclusive, as are
`--candidates-file` and `--candidates-json`.

### Candidates

- `add-candidate --election-id <ID> --candidate-id <N> --name <NAME>` — ids are `0-255`.

Candidates supplied at creation time use the shape `[{"id": 1, "name": "Alice"}]`.

### Registration tokens

- `generate-tokens --election-id <ID> --count <1-10000> [--output <PATH>]`
- `list-tokens --election-id <ID>`

Generated tokens are secret and displayed only once. Prefer `--output` to write them
to a file rather than leaving them in terminal scrollback or shell history.

## Output modes and exit codes

Human mode writes results to stdout and errors to stderr, colorized when stdout is a
TTY. `--json` emits exactly one JSON document to stdout on every path — success or
failure — so it is safe to pipe into `jq`.

Every response carries an `ok` boolean; failures add an `error` string. A command
exits `0` only when `ok` is `true`. Notably, `create-election` reports each candidate
outcome and still exits non-zero if any candidate failed to be added, and
`cancel-election` exits non-zero when the ec refuses the cancellation.

In `--json` mode destructive commands never prompt; they require `--yes` explicitly.

```sh
eccli --json list-elections | jq '.elections[].id'
```

See issue #1 for the full roadmap.
