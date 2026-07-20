# eccli

Friendly command-line client for the [Parallelia `ec`](https://github.com/Parallelia/ec)
Electoral Commission daemon. Written in Rust and **100% compatible with the current `ec`
gRPC Admin API** (`proto.admin.Admin`).

`eccli` lets an operator create and manage elections, add candidates, issue anonymous
registration tokens, and inspect state — all over the `ec` daemon's gRPC admin interface.

> **Experimental, unaudited software** — part of the Criptocracia trustless e-voting
> experiment. Do not use in real elections.

## Installation

Requires `protoc` (`apt install protobuf-compiler`).

```sh
cargo build --release
# binary at target/release/eccli
```

## Quick start

```sh
# Point at your ec daemon (default is http://127.0.0.1:50051)
eccli --server http://127.0.0.1:50051 check

# Create an election that starts in 1 minute and lasts 1 hour, with candidates
eccli create-election \
  --name "2025 Student Council" \
  --start-time 60 --duration 3600 \
  --rules-id plurality \
  --candidates-file candidates.json

# Issue 100 anonymous registration tokens and save them
eccli generate-tokens --election-id <ID> --count 100 --output tokens.txt
```

## Global options

| Option | Description |
|---|---|
| `--server <URL>` | ec gRPC endpoint. Default `http://127.0.0.1:50051`. Env: `EC_SERVER`. |
| `--token <TOKEN>` | Admin bearer token. Only needed when the ec sets `EC_ADMIN_TOKEN`. Env: `EC_ADMIN_TOKEN`. |
| `--json` | Emit machine-readable JSON (success and error) instead of human output. |
| `--yes`, `-y` | Skip confirmation prompts (required for destructive ops in scripts). |

## Commands

### `create-election`

Create a new election. Exactly one of `--duration` or `--end-time` is required.

```sh
eccli create-election --name "My Election" \
  --start-time 60 --duration 3600 \
  --rules-id plurality \
  --candidates-file candidates.json
```

| Parameter | Description |
|---|---|
| `--name, -n` | Election name. |
| `--start-time` | Relative seconds from now (`< 1_000_000_000`) or an absolute unix timestamp. |
| `--duration` | Length in seconds (mutually exclusive with `--end-time`). |
| `--end-time` | End as relative seconds or absolute unix ts (mutually exclusive with `--duration`). |
| `--rules-id` | Counting rules id. The ec ships `plurality` and `stv`. Default `plurality`. |
| `--candidates-file` | Path to a JSON file of candidates (see below). Optional. |
| `--candidates-json` | Candidates as an inline JSON string. Optional. |

Candidates are optional at creation; the client adds each one via a follow-up call and
reports per-candidate success. You can also add them later with `add-candidate`.

**`candidates.json` format** (ids must be `0–255`):

```json
[
  { "id": 1, "name": "Environmental Party" },
  { "id": 2, "name": "Tech Innovation Party" }
]
```

### `add-candidate`

```sh
eccli add-candidate --election-id <ID> --candidate-id 4 --name "Independent"
```

### `get-election`

```sh
eccli get-election --election-id <ID>
```

> **Note:** the ec's `GetElection` returns election **metadata only** (id, name, status,
> rules, window, RSA public key). Candidate lists and vote tallies are published to Nostr
> (kind 35000), not exposed over the gRPC admin API, so they are not shown here.

### `list-elections`

```sh
eccli list-elections
```

### `cancel-election`

Prompts for confirmation unless `--yes` is given.

```sh
eccli cancel-election --election-id <ID> --yes
```

### `generate-tokens`

Issue anonymous **registration tokens** for an election (see the model note below).

```sh
eccli generate-tokens --election-id <ID> --count 100 --output tokens.txt
```

| Parameter | Description |
|---|---|
| `--election-id, -e` | Target election. |
| `--count, -c` | Number of tokens (`1–10000`). |
| `--output, -o` | Optional file to write the raw tokens, one per line. |

Tokens are returned **once** and cannot be retrieved again — save them with `--output`
and distribute them securely. The file is written atomically and owner-only (`0600`); if
a symlink occupies the path it is replaced rather than followed. On non-Unix platforms,
where those permissions cannot be enforced, `--output` is refused and the tokens are
printed instead so none are lost.

### `list-tokens`

Show each token's id (a truncated SHA-256 the ec exposes) and whether it has been used.

```sh
eccli list-tokens --election-id <ID>
```

## The registration-token model (why there is no `add-voter`)

The previous CLI (now [`eccli_deprecated`](https://github.com/Parallelia/eccli_deprecated))
had `add-voter`/`list-voters`, which registered voters by **name + public key**. The current
`ec` deliberately does **not** support that, because linking a voter identity to the system
would undermine ballot anonymity.

Instead, the `ec` issues **anonymous registration tokens**. Voters redeem a token over Nostr
(NIP-59 Gift Wrap) to obtain a blind-RSA-signed voting credential, so the EC can verify that
a ballot came from an authorized voter **without ever learning which voter cast it**. In
`eccli` this maps to `generate-tokens` (issue) and `list-tokens` (audit usage).

## Authentication

When the `ec` is started with `EC_ADMIN_TOKEN`, every admin call must carry a bearer token.
Provide it with `--token` or the `EC_ADMIN_TOKEN` environment variable:

```sh
export EC_ADMIN_TOKEN=your-secret
eccli list-elections
```

Without a token against an auth-enabled server you'll get a clear `Unauthenticated` error.

## Time formats

`--start-time` and `--end-time` accept two forms:

- **Relative** (`< 1_000_000_000`): seconds from now — `60` = 1 minute, `3600` = 1 hour.
- **Absolute** (`>= 1_000_000_000`): a unix timestamp.

## Output modes and exit codes

Human mode writes results to stdout and errors to stderr, colorized when stdout is a
TTY. `--json` emits exactly one JSON document to stdout on every path — success or
failure — so it is safe to pipe into `jq`.

Every response carries an `ok` boolean; failures add an `error` string. A command
exits `0` only when `ok` is `true`. Notably, `create-election` reports each candidate
outcome and still exits non-zero if any candidate failed to be added, and
`cancel-election` exits non-zero when the ec refuses the cancellation.

Argument errors are part of that contract: with `--json`, a malformed invocation emits
an `{"ok": false, "error": ...}` document and exits `1` instead of clap's usage text.
Without `--json`, usage errors keep clap's behavior — human-readable text on stderr and
exit `2`. `--help` and `--version` always print normally and exit `0`.

In `--json` mode destructive commands never prompt; they require `--yes` explicitly.

```sh
eccli --json list-elections | jq '.elections[].id'
```

## Example workflow

```sh
set -euo pipefail

# 1. Create the election and capture its id.
#    `jq -e` exits non-zero when `.id` is absent, so a failed creation stops
#    here instead of leaving ID as "null" and cascading into the next steps.
ID=$(eccli --json create-election -n "Board 2025" --start-time 300 --duration 86400 \
  --rules-id plurality --candidates-file candidates.json | jq -er .id)

# 2. (Optionally) add more candidates
eccli add-candidate -e "$ID" -c 4 --name "Write-in"

# 3. Issue registration tokens for your voters
eccli generate-tokens -e "$ID" -c 500 -o tokens.txt

# 4. Audit token usage during/after the election
eccli list-tokens -e "$ID"
```

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo llvm-cov --summary-only   # coverage (needs cargo-llvm-cov + llvm-tools-preview)
```

## License

MIT — see [LICENSE](LICENSE).
