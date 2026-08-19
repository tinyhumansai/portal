---
name: portal
description: Use when you need a web search, a deep research run, an LLM or embedding or transcription call, image or video generation, market or places or calendar data, a third-party SaaS tool, file storage, an outbound call, or an agent session — Portal reaches every TinyHumans provider, search engine, and model through one catalog, as a CLI (`portal`) or an MCP server (`portal mcp`).
---

# Portal

Portal is one door to every capability the TinyHumans backend offers: search
engines, models, media generation, market and location data, third-party tool
execution, file storage, messaging, and agent orchestration. You do not need to
know any routes. You search a catalog, read a capability's arguments, and
invoke it.

## Setup

Portal needs one credential:

```sh
export TINYHUMANS_API_KEY=...     # or TINYHUMANS_TOKEN=<user bearer token>
portal status                     # backend, credential, reachability
```

`TINYHUMANS_BASE_URL` overrides the backend (staging, or a local server).

## The loop

Always work in this order. Guessing a capability id or an argument name wastes
a call; the catalog answers both offline and instantly.

```sh
portal catalog                        # 1. what families exist
portal search "transcribe audio"      # 2. find the capability
portal describe models.transcribe     # 3. read its arguments and example
portal call models.transcribe --arg file=./interview.mp3
```

`portal describe` ends with a ready-to-run `portal call` line. Copy it.

## Invoking

```sh
# Repeated --arg, one per argument. Strings need no quoting.
portal call search.web \
  --arg objective="who ships the fastest rust http client" \
  --arg search_queries='["fastest rust http client"]'

# Or pass the whole argument object as JSON.
portal call models.chat --args-json '{
  "model": "tiny-1",
  "messages": [{"role": "user", "content": "summarise this changelog"}]
}'

# See the request without sending it.
portal call media.image --arg prompt="a tiny lighthouse" --dry-run

# Capabilities that return bytes need an output path.
portal call files.download --arg file_id=f_123 --out ./report.pdf
```

Arguments accept either spelling — `search_queries` or `searchQueries`. An
unknown argument, a missing required one, or a wrong type is rejected locally,
before any network call, with a message naming the argument.

## Shorthands

```sh
portal web "what changed in tokio 1.40"   # search.web
portal chat "explain this stack trace"    # models.chat
portal models                             # models.list
portal raw GET /teams/me/usage            # any public route the catalog misses
```

## As an MCP server

`portal mcp` speaks the Model Context Protocol on stdio and exposes four tools:

| Tool | Use it to |
| --- | --- |
| `portal_search` | find capabilities in plain words — start here |
| `portal_describe` | read one capability's arguments and JSON Schema |
| `portal_invoke` | call a capability; `save_to` writes a byte response to a file |
| `portal_status` | check the backend, the credential, and reachability |

Register it in any MCP client:

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

## Categories

| Category | What is in it |
| --- | --- |
| `search` | web search, agentic browsing, page fetch, places, gifs |
| `research` | deep research runs, web datasets, enrichment |
| `models` | chat, completion, embeddings, speech, transcription |
| `media` | image and video generation, animated assets |
| `data` | market and FX data, places details, calendar, on-chain routes |
| `automation` | Composio tools, Apify actors, triggers, webhooks |
| `files` | upload, download, share, and account storage usage |
| `messaging` | chat channels and outbound voice calls |
| `agents` | Medulla sessions and tasks, orchestration runs, companies |
| `account` | identity, teams, credits, billing, pricing, quotas |

## Rules

- Search the catalog before inventing an id; ids are stable and discoverable.
- Read `portal describe` before a first call to an unfamiliar capability.
- Long-running work (deep research, datasets, media generation) starts one
  capability and polls another — `research.start` then `research.status`, and
  `research.result` when it is done. Do not busy-poll; these take minutes.
- Anything outside `GET` spends credits or changes state. Say what you are
  about to do before calling it on someone's behalf.
- `portal status` first when a call fails unexpectedly; a missing credential
  and an unreachable backend look identical from inside one failed call.
