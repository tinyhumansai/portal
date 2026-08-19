# 3. Expose four MCP tools, not one per capability

- **Status:** Accepted
- **Date:** 2026-08-19

## Context

The Model Context Protocol advertises tools up front: a client calls
`tools/list` once and the definitions sit in the model's context for the rest
of the session. Portal has 202 capabilities. Exposing each as its own tool is
the literal reading of the protocol and would let a model call any capability
directly, with a schema it can see.

It would also put 202 tool definitions — names, descriptions, and JSON Schemas,
tens of thousands of tokens — into every conversation, most of them irrelevant
to the task at hand. Tool-selection accuracy falls as the list grows, and the
context they consume is context the actual work does not get.

## Decision

Expose four tools and make the catalog searchable at runtime:

| Tool | Purpose |
| --- | --- |
| `portal_search` | find capabilities in plain words |
| `portal_describe` | read one capability's arguments and JSON Schema |
| `portal_invoke` | call a capability with JSON arguments |
| `portal_status` | check configuration, credentials, and reachability |

`portal_describe` returns the same JSON Schema a per-capability tool would have
advertised, so the model still sees a precise contract — it just fetches the
one it needs instead of carrying all of them. The handshake's `instructions`
field names this path, so a model knows to search first.

## Consequences

- A capability call costs one extra round trip: search or describe, then
  invoke. In exchange the tool list stays four entries long no matter how far
  the backend grows.
- Growth is free. A regenerated catalog with fifty new capabilities changes no
  tool definitions.
- Argument errors are caught by Portal rather than by the client's schema
  validation, so `portal_invoke` must — and does — return precise, local
  argument errors as tool results.
- A client that filters or renames tools sees four stable names.
