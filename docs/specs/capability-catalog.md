# Capability catalog

- **Status:** Implemented
- **Owner:** Portal maintainers

## Problem

The TinyHumans backend exposes roughly two hundred public operations across
twenty-one namespaces: web search, deep research, models, media generation,
market and location data, third-party tool execution, file storage, messaging,
agent orchestration, and account management. An agent that wants one of them
faces three obstacles at once — it does not know the operation exists, it does
not know what the operation is called, and it does not know what arguments the
operation takes.

The vendored SDK solves none of these. It is an excellent typed client for a
caller who already knows the route, and useless to a caller who does not.

## Goals

- An agent with no prior knowledge can go from an intention in plain words to a
  correct call, using only the tool in front of it.
- One description of each operation, shared by every surface, so the CLI, the
  MCP server, the library, and the Agent Skill cannot disagree.
- Argument mistakes fail locally, before any network call, naming the argument
  at fault.
- Nothing Portal exposes can exceed what the vendored SDK permits.

## Non-goals

- Replacing the SDK's transport, authentication, or envelope handling.
- Exposing administrative routes. They are excluded at the SDK layer and
  excluded again here.
- Streaming. Portal is request and response; the SDK's SSE and Socket.IO
  surfaces stay in the SDK.
- Hiding the backend. `Portal::raw` and `portal raw` stay available for routes
  the catalog does not model yet.

## Proposed behaviour

### The capability

A capability is one callable backend operation, carrying everything a caller
needs to invoke it without further lookup:

| Field | Meaning |
| --- | --- |
| `id` | the stable identifier a caller types, such as `search.web` |
| `aliases` | other identifiers that resolve to the same capability |
| `category` | one of ten families, for discovery |
| `group`, `provider` | the catalog family and the product actually serving it |
| `summary` | one line, taken from the deployed contract |
| `method`, `path` | the route, with `{placeholder}` path parameters |
| `params` | every argument: name, wire name, location, type, requiredness |
| `body`, `envelope`, `response`, `auth` | how the request and response are shaped |

Identifiers are derived deterministically from the route and then, for the
capabilities an agent reaches for first, replaced by a curated short id with the
derived one kept as an alias. **An identifier is a public contract**: renaming
one breaks every agent, Skill, and script that used it.

### Discovery

- `catalog::find` resolves an id or alias.
- `catalog::search` ranks the catalog against plain words. Every term must
  match; an identifier match outranks a summary match, which outranks a
  provider or path match, which outranks a parameter-name match.
- `catalog::in_category`, `in_group`, `providers`, and `groups` enumerate.

Discovery is offline and needs no credential.

### Invocation

`invoke::plan` is a pure function from a capability and a JSON object to a
request. It rejects, in order: arguments that are not an object, arguments the
capability does not declare, absent required arguments, and values of the wrong
JSON type. It then substitutes and percent-encodes path parameters, collects
query parameters, and builds either a JSON body or a multipart form.

`Portal` adds three guards before sending: an unknown identifier, a missing
credential for a capability that needs one, and a byte-returning capability
invoked through the JSON entry point.

### Arguments

Each parameter has an agent-facing `snake_case` name and the backend's own wire
name. Both are accepted. A capability whose body the contract does not describe
carries a single free-form `body` object parameter, which is sent as the body
rather than nested inside one.

### Bytes

Downloads and generated audio return bytes. They are never inlined into a JSON
result — an audio file rendered as encoded text would swamp the caller's
context. `Portal::invoke_bytes`, `portal call --out <path>`, and the MCP
`save_to` argument are the ways to get them.

## Invariants

1. Portal's route set is a subset of the vendored SDK's public-route allowlist.
   The generator enforces this by intersection; there is no other path to the
   network.
2. `src/catalog/generated.rs` and `api/portal.catalog.json` are generated. CI
   runs the generator with `--check` and fails on drift.
3. Every path placeholder has a required path parameter.
4. Only `/openai/*` routes bypass the response envelope.
5. Identifiers and aliases are globally unique.
6. Credentials never appear in `Debug` output, in `portal status`, or in an MCP
   response.
7. Generation reads a committed contract snapshot, not the live backend, so a
   deployment cannot fail an unrelated pull request.

Invariants 1 and 3 to 5 are asserted by tests in `src/catalog/test.rs` and
`tests/public_api.rs`; 2 by CI; 6 by tests in `src/config/test.rs`,
`src/client/test.rs`, and `src/mcp/test.rs`.

## Acceptance criteria

- `portal search "<intention>"` returns the right capability for each category.
- `portal describe <id>` prints arguments and an invocation that runs as shown.
- A wrong, missing, or misspelled argument fails without a network call and
  names the argument.
- The MCP server answers `initialize`, `tools/list`, and `tools/call` and
  reports tool failures as tool results rather than protocol errors.
- Every file under `src/` keeps at least 90% line coverage.

## Open questions

- Streaming responses (`models.chat` with `stream: true`, and the Medulla
  session stream) are declared in the catalog but returned buffered. A
  streaming entry point is future work.
- Long-running capabilities (deep research, datasets, media generation) are
  start-and-poll. Portal does not yet offer a wait helper.
