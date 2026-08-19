# Portal

One agent-friendly door to every provider, search engine, model, and agent
runtime the TinyHumans backend offers — as a Rust SDK, a CLI, an MCP server,
and an Agent Skill.

Portal wraps the [TinyHumans SDK](https://github.com/tinyhumansai/sdk), vendored
at `vendor/sdk`, in a **capability catalog**: 202 backend operations, each with
a stable identifier, a category, typed arguments, and documentation taken
straight from the deployed OpenAPI contract. An agent that has never seen this
API can search the catalog in plain words, read a capability's arguments, and
invoke it — without knowing a single route.

```sh
portal search "transcribe audio"      # find it
portal describe models.transcribe     # read its arguments
portal call models.transcribe --arg file=./interview.mp3
```

## Install

```sh
cargo build --release --bin portal      # target/release/portal
export TINYHUMANS_API_KEY=...           # or TINYHUMANS_TOKEN=<bearer token>
portal status
```

Release builds for Linux, macOS, and Windows are attached to every GitHub
release, together with the Agent Skill.

## The four surfaces

They are all the same catalog, so they cannot drift apart.

| Surface | Entry point | Use it when |
| --- | --- | --- |
| CLI | `portal` | a human or a shell-driven agent is at the keyboard |
| MCP server | `portal mcp` | an MCP client should discover and call capabilities |
| Agent Skill | `skills/portal/SKILL.md` | an agent needs to learn Portal in-context |
| Rust library | `portal::Portal` | another Rust program embeds the whole surface |

### CLI

```sh
portal catalog                       # the ten categories and their sizes
portal catalog --category search     # what is in one of them
portal search "deep research"        # rank the catalog against a question
portal describe research.start       # arguments, route, and an example call

portal call search.web \
  --arg objective="who ships the fastest rust http client" \
  --arg search_queries='["fastest rust http client"]'

portal call media.image --arg prompt="a tiny lighthouse" --dry-run
portal call files.download --arg file_id=f_123 --out ./report.pdf

portal web "what changed in tokio 1.40"   # shorthand for search.web
portal chat "explain this stack trace"    # shorthand for models.chat
portal models                             # shorthand for models.list
portal raw GET /teams/me/usage            # any public route the catalog misses
```

Arguments accept either spelling — `search_queries` or `searchQueries`. An
unknown argument, a missing required one, or a wrong type is rejected locally,
before any network call, naming the argument at fault.

### MCP server

`portal mcp` speaks line-delimited JSON-RPC on stdio and exposes four tools:
`portal_search`, `portal_describe`, `portal_invoke`, and `portal_status`. Four
rather than 202, because a tool definition per capability would crowd out the
conversation it is meant to serve — the catalog is searched at runtime instead.

```json
{
  "mcpServers": {
    "portal": {
      "command": "portal",
      "args": ["mcp"],
      "env": { "TINYHUMANS_API_KEY": "..." }
    }
  }
}
```

### Rust library

```rust
use portal::{catalog, Portal};
use serde_json::json;

// Discovery needs no credential and no network.
let web = catalog::find("search.web").expect("the web search capability");
assert_eq!(web.provider, "Parallel AI");

# async fn run() -> Result<(), portal::Error> {
let results = Portal::from_env()
    .invoke("search.web", &json!({
        "objective": "who ships the fastest rust http client",
        "search_queries": ["fastest rust http client"],
    }))
    .await?;
# Ok(())
# }
```

## What is in the catalog

| Category | Count | What is in it |
| --- | --- | --- |
| `search` | 8 | web search, agentic browsing, page fetch, places, gifs |
| `research` | 7 | deep research runs, web datasets, enrichment |
| `models` | 6 | chat, completion, embeddings, speech, transcription |
| `media` | 10 | image and video generation, animated assets |
| `data` | 14 | market and FX data, place details, calendar, on-chain routes |
| `automation` | 20 | Composio tools, Apify actors, triggers, webhooks |
| `files` | 9 | upload, download, share, storage usage |
| `messaging` | 8 | chat channels and outbound voice calls |
| `agents` | 43 | Medulla sessions and tasks, orchestration, companies |
| `account` | 77 | identity, teams, credits, billing, pricing, quotas |

Run `portal catalog` for the live counts, or read
[`api/portal.catalog.json`](api/portal.catalog.json) for the whole table.

## Configuration

| Variable | Meaning | Default |
| --- | --- | --- |
| `TINYHUMANS_API_KEY` | Long-lived API key, sent as `x-api-key` | unset |
| `TINYHUMANS_TOKEN` | User bearer token, sent as `Authorization` | unset |
| `TINYHUMANS_BASE_URL` | Backend origin | `https://api.tinyhumans.ai` |

One credential is required for everything except the public health check.
Credentials are never printed: `portal status` and every debug rendering report
the credential *kind*, not the secret.

## How the catalog is built

`scripts/sync-catalog.mjs` reads the committed contract snapshot at
`api/tinyhumans.openapi.json`, intersects it with the vendored SDK's
public-route allowlist — so Portal can never expose a route the SDK itself
refuses to send, including every administrative one — and generates
`src/catalog/generated.rs` and `api/portal.catalog.json`.

```sh
node scripts/sync-catalog.mjs --fetch    # refresh the snapshot from the backend
node scripts/sync-catalog.mjs            # regenerate from the snapshot
node scripts/sync-catalog.mjs --check    # what CI runs
```

Editorial metadata — which family a route belongs to, which product serves it,
and the short id an agent types — lives in `scripts/curation.mjs`. Everything
else comes from the contract.

## Development

```sh
git submodule update --init --recursive

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

See [`AGENTS.md`](AGENTS.md) for the full working agreement, and
[`docs/`](docs/) for the specification behind the catalog.
