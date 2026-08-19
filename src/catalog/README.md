# `catalog`

The compiled-in table of everything Portal can invoke, and discovery over it.

## Design

`generated.rs` holds a `&'static [Capability]` written by
`scripts/sync-catalog.mjs` from the committed OpenAPI snapshot, intersected with
the vendored SDK's public-route allowlist. It is data only: no logic, no
allocation, no I/O. Because the table is `'static`, a lookup cannot fail for
want of a file, and every surface — library, CLI, MCP server — shares one
description of each operation.

`types.rs` is the vocabulary that table is written in. `mod.rs` is the discovery
layer: `find`, `search`, `in_category`, `in_group`, `providers`, `groups`.

## Public surface

- [`Capability`] — one callable operation, with its route, parameters, and
  transport details. [`Capability::input_schema`] renders the JSON Schema both
  `portal describe --json` and the MCP server hand to a model.
- [`Category`] — the ten families, with slugs, descriptions, and parsing.
- [`find`] — resolve an identifier or alias.
- [`search`] — rank the table against plain words, best match first.

## Constraints

- **Never edit `generated.rs`.** Change `scripts/curation.mjs` and regenerate;
  CI runs `node scripts/sync-catalog.mjs --check`.
- **Identifiers are a public contract.** They appear in the Agent Skill, in
  documentation, and in whatever agents and scripts have already been written
  against them. `tests/public_api.rs` pins the flagship ones.
- Search requires every term to match. A query that matches nothing returns
  nothing rather than the whole catalog; only an empty query returns everything.
