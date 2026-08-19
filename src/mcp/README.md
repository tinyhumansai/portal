# `mcp`

A Model Context Protocol server over stdio, serving the capability catalog.

## Design

Four tools — `portal_search`, `portal_describe`, `portal_invoke`,
`portal_status` — rather than one per capability. The reasoning is in
[ADR 3](../../docs/adr/0003-four-mcp-tools.md): 202 tool definitions would
consume more context than the work they support, so the catalog is searched at
runtime instead of enumerated at handshake time.

`types.rs` holds the JSON-RPC 2.0 envelope. It is written out rather than
depended on: the surface is four methods over line-delimited stdio, small enough
that a hand-written envelope stays auditable and testable without a runtime.

`Server::handle` takes one line and returns the line to write back, or `None`
for a notification. `Server::serve` is the loop around it. Everything is
therefore testable by handing it a string.

## Public surface

- [`Server::new`] — bind a server to a configured `Portal`.
- [`Server::serve`] — read stdin, write stdout, until stdin closes.
- [`Server::handle`] — one request line in, one response line out.
- [`PROTOCOL_VERSION`] — the MCP revision implemented.

## Constraints

- **Nothing but protocol goes to stdout.** A stray `println!` corrupts the
  stream; diagnostics belong on stderr.
- **A failed capability call is a tool result with `isError: true`, not a
  JSON-RPC error.** The specification reserves protocol errors for malformed
  requests; a model is meant to read a tool failure and react to it.
- **Bytes are never inlined.** A byte-returning capability requires `save_to`;
  the tool answers with the path and the length it wrote.
- Credentials are never echoed. `portal_status` reports the credential kind.
