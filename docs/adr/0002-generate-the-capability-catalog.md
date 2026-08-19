# 2. Generate the capability catalog from the deployed contract

- **Status:** Accepted
- **Date:** 2026-08-19

## Context

Portal's whole value is a catalog of every backend capability, described well
enough for an agent to invoke one it has never seen. The backend publishes
roughly two hundred public operations, each with parameters, types, and
summaries, and it deploys independently of this repository.

Hand-writing that catalog was the obvious first move and the wrong one. Two
hundred entries written by hand are two hundred chances to mistype a field name
an agent will then send, and they begin drifting from the backend the day they
are written. The vendored SDK already faced this and answered it the same way:
`scripts/sync-openapi.mjs` generates its route registry.

## Decision

Generate the catalog. `scripts/sync-catalog.mjs` reads the OpenAPI contract,
intersects it with the SDK's public-route allowlist, and writes
`src/catalog/generated.rs` and `api/portal.catalog.json`. Neither file is
hand-edited, and CI runs the generator with `--check`.

Three consequences of that decision are worth stating explicitly:

- **Editorial metadata is separated, not eliminated.** Which family a route
  belongs to, which product serves it, and the short id an agent types are
  judgements a contract cannot make. They live in `scripts/curation.mjs`, where
  they are reviewable on their own.
- **Identifiers are derived deterministically**, from the group and path, with
  a verb suffix added only to separate colliding routes. The same contract
  always produces the same ids, so regeneration is not a renaming event.
- **The input is a committed snapshot**, `api/tinyhumans.openapi.json`, not the
  live backend. Refreshing it is a deliberate `--fetch` commit.

## Consequences

- The catalog cannot silently disagree with the backend about a parameter name,
  and cannot exceed what the SDK will send.
- Adding a capability is a regeneration, not a code change; reviewing one is
  reading a data diff.
- A backend deployment cannot turn an unrelated pull request red, because CI
  compares against the snapshot. The cost is that the snapshot can lag, and
  somebody has to run `--fetch`.
- Node is required to change the catalog, though not to build or use Portal.
