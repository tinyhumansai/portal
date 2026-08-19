#!/usr/bin/env node
//
// Regenerate the Portal capability catalog from the deployed backend contract.
//
//   node scripts/sync-catalog.mjs            # regenerate from the snapshot
//   node scripts/sync-catalog.mjs --check    # fail if the output is stale
//   node scripts/sync-catalog.mjs --fetch    # refresh the snapshot, then both
//   node scripts/sync-catalog.mjs --spec f   # read some other local document
//
// Three things are generated and committed:
//
//   api/tinyhumans.openapi.json  the pinned public contract snapshot
//   src/catalog/generated.rs     the `CAPABILITIES` table the crate compiles in
//   api/portal.catalog.json      the same table as JSON, for non-Rust readers
//
// The snapshot is what makes CI hermetic: `--check` compares the generated
// files against a committed contract rather than against whatever the backend
// is serving this minute, so a deployment cannot turn someone else's pull
// request red. Refreshing it is a deliberate act — `--fetch`, in its own
// commit, with the catalog diff visible for review.
//
// The route set is intersected with the vendored SDK's public-route allowlist,
// so Portal can never expose a route the SDK itself refuses to send. Editorial
// metadata (groups, providers, short ids) lives in `curation.mjs`.

import { spawn } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import process from "node:process";

import {
  ALIASES,
  BINARY_ROUTES,
  CATEGORY_OVERRIDES,
  GROUPS,
  GROUP_META,
  MULTIPART_ROUTES,
  PUBLIC_ROUTES,
  SUPPLEMENTAL_BODIES,
} from "./curation.mjs";

const SPEC_URL = "https://api.tinyhumans.ai/swagger.json";
const SPEC_PATH = resolve("api/tinyhumans.openapi.json");
const SDK_ROUTES_PATH = resolve("vendor/sdk/src/generated_public_routes.rs");
const RUST_PATH = resolve("src/catalog/generated.rs");
const JSON_PATH = resolve("api/portal.catalog.json");
const HTTP_METHODS = new Set(["delete", "get", "patch", "post", "put"]);

const args = process.argv.slice(2);
const check = args.includes("--check");
const fetchSnapshot = args.includes("--fetch");
const specIndex = args.indexOf("--spec");
const specPath = specIndex === -1 ? null : args[specIndex + 1];

main().catch((error) => {
  console.error(`sync-catalog: ${error.message}`);
  process.exit(1);
});

async function main() {
  const spec = await loadSpec();
  const allowed = await readSdkRoutes();
  const capabilities = buildCapabilities(spec, allowed);
  await emit(RUST_PATH, await rustfmt(renderRust(capabilities)));
  await emit(JSON_PATH, `${JSON.stringify(toManifest(spec, capabilities), null, 2)}\n`);
  console.log(
    `sync-catalog: ${capabilities.length} capabilities across ${
      new Set(capabilities.map((c) => c.group)).size
    } groups${check ? " (checked)" : ""}`,
  );
}

// Read the contract: an explicit file, a fresh fetch that also updates the
// committed snapshot, or the snapshot itself.
async function loadSpec() {
  if (specPath) {
    return JSON.parse(await readFile(resolve(specPath), "utf8"));
  }
  if (fetchSnapshot) {
    const response = await fetch(SPEC_URL);
    if (!response.ok) {
      throw new Error(`GET ${SPEC_URL} returned ${response.status}`);
    }
    const spec = await response.json();
    await emit(SPEC_PATH, `${JSON.stringify(spec, null, 2)}\n`);
    return spec;
  }
  const snapshot = await readFile(SPEC_PATH, "utf8").catch(() => null);
  if (snapshot === null) {
    throw new Error(`${SPEC_PATH} is missing; run node scripts/sync-catalog.mjs --fetch`);
  }
  return JSON.parse(snapshot);
}

// The SDK's allowlist is the source of truth for what may be called at all.
async function readSdkRoutes() {
  const source = await readFile(SDK_ROUTES_PATH, "utf8");
  const section = (name) => {
    const start = source.indexOf(`${name}: &[(&str, &str)] = &[`);
    if (start === -1) {
      throw new Error(`${name} not found in ${SDK_ROUTES_PATH}`);
    }
    const end = source.indexOf("];", start);
    const routes = new Set();
    for (const match of source.slice(start, end).matchAll(/\("([A-Z]+)", "([^"]+)"\)/g)) {
      routes.add(`${match[1]} ${match[2]}`);
    }
    return routes;
  };
  const publicRoutes = section("PUBLIC_ROUTES");
  for (const route of section("UNEXPOSED_ROUTES")) {
    publicRoutes.delete(route);
  }
  // Inbound webhook receivers are the backend calling us, not us calling it.
  return new Set([...publicRoutes].filter((route) => !route.includes(" /webhooks/") || route.includes("/webhooks/core")));
}

function buildCapabilities(spec, allowed) {
  const seen = new Set();
  const capabilities = [];
  for (const [path, item] of Object.entries(spec.paths ?? {})) {
    for (const [verb, operation] of Object.entries(item)) {
      if (!HTTP_METHODS.has(verb)) continue;
      const method = verb.toUpperCase();
      const route = `${method} ${path}`;
      if (!allowed.has(route)) continue;
      seen.add(route);
      capabilities.push(describe(spec, method, path, operation));
    }
  }
  // Public routes the deployed document does not describe still belong in the
  // catalog; they invoke through a free-form body instead of a typed one.
  for (const route of allowed) {
    if (seen.has(route)) continue;
    const [method, path] = splitRoute(route);
    capabilities.push(describe(spec, method, path, null));
  }
  assignIds(capabilities);
  capabilities.sort((a, b) => a.id.localeCompare(b.id));
  return capabilities;
}

function splitRoute(route) {
  const space = route.indexOf(" ");
  return [route.slice(0, space), route.slice(space + 1)];
}

function describe(spec, method, path, operation) {
  const route = `${method} ${path}`;
  const group = groupFor(path);
  const [provider, category] = GROUP_META[group] ?? ["TinyHumans", "account"];
  const multipartField = MULTIPART_ROUTES[route] ?? null;
  return {
    route,
    method,
    path,
    group,
    provider,
    category: CATEGORY_OVERRIDES[route] ?? category,
    base: baseId(group, path),
    alias: ALIASES[route] ?? null,
    summary: summaryFor(route, operation),
    params: collectParams(spec, path, operation, route, multipartField),
    bodyKind: multipartField ? "Multipart" : hasBody(method, operation, route) ? "Json" : "None",
    envelope: path.startsWith("/openai/") ? "Raw" : "Unwrap",
    response: BINARY_ROUTES.has(route) ? "Binary" : "Json",
    auth: PUBLIC_ROUTES.has(route) ? "Public" : "Required",
  };
}

function summaryFor(route, operation) {
  const summary = operation?.summary ?? operation?.description ?? null;
  if (summary) return summary.replace(/\s+/g, " ").trim();
  return SUPPLEMENTAL_BODIES[route] ?? `${route} (undocumented public route)`;
}

function hasBody(method, operation, route) {
  if (method === "GET" || method === "DELETE") return false;
  if (operation?.requestBody) return true;
  return method !== "GET";
}

function groupFor(path) {
  if (path === "/") return "health";
  let best = null;
  for (const [prefix, group] of GROUPS) {
    if (path === prefix || path.startsWith(`${prefix}/`) || path.startsWith(prefix)) {
      if (!best || prefix.length > best[0].length) best = [prefix, group];
    }
  }
  return best ? best[1] : "misc";
}

function baseId(group, path) {
  const prefix = GROUPS.find(([, name]) => name === group)?.[0] ?? "";
  const rest = path
    .slice(path.startsWith(prefix) ? prefix.length : 0)
    .split("/")
    .filter((segment) => segment && !segment.startsWith("{"))
    .filter((segment) => segment !== "v1")
    .map(snake);
  // `/agent-integrations/file-storage/files` is the files group's own
  // collection, not `files.files`.
  if (rest[0] === group) rest.shift();
  return [group, ...rest].join(".");
}

// Ids stay identical run to run: the base name wins when it is unique, and
// colliding routes are separated by the verb their method implies.
function assignIds(capabilities) {
  const counts = new Map();
  for (const capability of capabilities) {
    counts.set(capability.base, (counts.get(capability.base) ?? 0) + 1);
  }
  const used = new Set();
  for (const capability of capabilities) {
    const derived =
      counts.get(capability.base) === 1
        ? capability.base
        : `${capability.base}.${verbFor(capability)}`;
    capability.id = capability.alias ?? derived;
    capability.aliases = capability.alias && capability.alias !== derived ? [derived] : [];
    if (used.has(capability.id)) {
      throw new Error(`duplicate capability id ${capability.id} for ${capability.route}`);
    }
    used.add(capability.id);
  }
}

function verbFor({ method, path }) {
  const endsWithParam = path.endsWith("}");
  switch (method) {
    case "GET":
      return endsWithParam ? "get" : "list";
    case "POST":
      return "create";
    case "PUT":
      return "replace";
    case "PATCH":
      return "update";
    default:
      return "delete";
  }
}

function collectParams(spec, path, operation, route, multipartField) {
  const params = [];
  const declared = new Set();
  for (const raw of operation?.parameters ?? []) {
    const parameter = deref(spec, raw);
    if (parameter.in !== "path" && parameter.in !== "query") continue;
    declared.add(parameter.name);
    params.push({
      name: snake(parameter.name),
      wire: parameter.name,
      location: parameter.in === "path" ? "Path" : "Query",
      kind: kindOf(deref(spec, parameter.schema ?? {})),
      required: parameter.in === "path" ? true : Boolean(parameter.required),
      description: describeSchema(deref(spec, parameter.schema ?? {}), parameter.description),
    });
  }
  // A path template always needs every one of its placeholders, whether or not
  // the document bothered to declare them.
  for (const match of path.matchAll(/\{([^}]+)\}/g)) {
    if (declared.has(match[1])) continue;
    params.push({
      name: snake(match[1]),
      wire: match[1],
      location: "Path",
      kind: "String",
      required: true,
      description: `The ${match[1]} path segment.`,
    });
  }
  if (multipartField) {
    params.push({
      name: multipartField,
      wire: multipartField,
      location: "Body",
      kind: "File",
      required: true,
      description: "Path to a local file to upload.",
    });
  }
  const schema = bodySchema(spec, operation);
  if (schema && schema.type === "object" && schema.properties) {
    const required = new Set(schema.required ?? []);
    for (const [name, raw] of Object.entries(schema.properties)) {
      const property = deref(spec, raw);
      params.push({
        name: snake(name),
        wire: name,
        location: "Body",
        kind: kindOf(property),
        required: required.has(name),
        description: describeSchema(property, property.description),
      });
    }
  } else if (!multipartField && hasBody(routeMethod(route), operation, route)) {
    params.push({
      name: "body",
      wire: "body",
      location: "Body",
      kind: "Object",
      required: Boolean(operation?.requestBody?.required),
      description: "Raw JSON request body, forwarded to the backend as-is.",
    });
  }
  return dedupe(params);
}

function routeMethod(route) {
  return route.slice(0, route.indexOf(" "));
}

function dedupe(params) {
  const byName = new Map();
  for (const param of params) {
    if (!byName.has(param.name)) byName.set(param.name, param);
  }
  return [...byName.values()];
}

function bodySchema(spec, operation) {
  const content = operation?.requestBody?.content ?? {};
  const media = content["application/json"] ?? content["multipart/form-data"];
  return media?.schema ? deref(spec, media.schema) : null;
}

function deref(spec, node) {
  let current = node;
  for (let hops = 0; current && current.$ref && hops < 16; hops += 1) {
    const segments = current.$ref.replace(/^#\//, "").split("/");
    current = segments.reduce((value, segment) => value?.[segment], spec);
  }
  return current ?? {};
}

function kindOf(schema) {
  switch (schema.type) {
    case "string":
      return "String";
    case "integer":
      return "Integer";
    case "number":
      return "Number";
    case "boolean":
      return "Boolean";
    case "array":
      return "Array";
    case "object":
      return "Object";
    default:
      return "Any";
  }
}

function describeSchema(schema, description) {
  const parts = [];
  if (description) parts.push(description.replace(/\s+/g, " ").trim());
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    parts.push(`One of: ${schema.enum.join(", ")}.`);
  }
  if (schema.type === "array" && schema.items?.type) {
    parts.push(`Array of ${schema.items.type}.`);
  }
  if (schema.default !== undefined) parts.push(`Defaults to ${JSON.stringify(schema.default)}.`);
  return parts.join(" ");
}

function snake(value) {
  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[-\s.]+/g, "_")
    .toLowerCase();
}

function rust(value) {
  return JSON.stringify(value);
}

function renderRust(capabilities) {
  const lines = [
    "// Generated by scripts/sync-catalog.mjs. Do not edit.",
    "//! The capability table, derived from the deployed backend contract.",
    "//!",
    "//! Regenerate with `node scripts/sync-catalog.mjs`; CI runs the same script",
    "//! with `--check` so a drifted table fails the build instead of the caller.",
    "",
    "use super::types::{Auth, BodyKind, Capability, Category, Envelope, Method, Param, ParamIn, ParamKind, ResponseKind};",
    "",
    "/// Every capability Portal can invoke, sorted by identifier.",
    "pub(crate) const CAPABILITIES: &[Capability] = &[",
  ];
  for (const capability of capabilities) {
    lines.push("    Capability {");
    lines.push(`        id: ${rust(capability.id)},`);
    lines.push(
      capability.aliases.length === 0
        ? "        aliases: &[],"
        : `        aliases: &[${capability.aliases.map(rust).join(", ")}],`,
    );
    lines.push(`        category: Category::${pascal(capability.category)},`);
    lines.push(`        group: ${rust(capability.group)},`);
    lines.push(`        provider: ${rust(capability.provider)},`);
    lines.push(`        summary: ${rust(capability.summary)},`);
    lines.push(`        method: Method::${pascal(capability.method.toLowerCase())},`);
    lines.push(`        path: ${rust(capability.path)},`);
    if (capability.params.length === 0) {
      lines.push("        params: &[],");
    } else {
      lines.push("        params: &[");
      for (const param of capability.params) {
        lines.push("            Param {");
        lines.push(`                name: ${rust(param.name)},`);
        lines.push(`                wire_name: ${rust(param.wire)},`);
        lines.push(`                location: ParamIn::${param.location},`);
        lines.push(`                kind: ParamKind::${param.kind},`);
        lines.push(`                required: ${param.required},`);
        lines.push(`                description: ${rust(param.description)},`);
        lines.push("            },");
      }
      lines.push("        ],");
    }
    lines.push(`        body: BodyKind::${capability.bodyKind},`);
    lines.push(`        envelope: Envelope::${capability.envelope},`);
    lines.push(`        response: ResponseKind::${capability.response},`);
    lines.push(`        auth: Auth::${capability.auth},`);
    lines.push("    },");
  }
  lines.push("];", "");
  return lines.join("\n");
}

function pascal(value) {
  return value
    .split(/[_-]/)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

function toManifest(spec, capabilities) {
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    name: "portal-capabilities",
    description:
      "Every backend capability Portal exposes, derived from the deployed TinyHumans OpenAPI contract and the vendored SDK's public-route allowlist.",
    source: {
      url: SPEC_URL,
      title: spec.info?.title ?? null,
      version: spec.info?.version ?? null,
    },
    capabilityCount: capabilities.length,
    categories: [...new Set(capabilities.map((c) => c.category))].sort(),
    capabilities: capabilities.map((capability) => ({
      id: capability.id,
      aliases: capability.aliases,
      category: capability.category,
      group: capability.group,
      provider: capability.provider,
      summary: capability.summary,
      route: capability.route,
      auth: capability.auth,
      response: capability.response,
      params: capability.params.map((param) => ({
        name: param.name,
        in: param.location.toLowerCase(),
        type: param.kind.toLowerCase(),
        required: param.required,
        description: param.description,
      })),
    })),
  };
}

// The generated Rust is formatted here rather than by a follow-up `cargo fmt`,
// so `--check` compares against exactly what lands on disk and the two checks
// cannot disagree.
function rustfmt(source) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn("rustfmt", ["--edition", "2024", "--emit", "stdout", "--quiet"]);
    let out = "";
    let err = "";
    child.stdout.on("data", (chunk) => {
      out += chunk;
    });
    child.stderr.on("data", (chunk) => {
      err += chunk;
    });
    child.on("error", (error) => reject(new Error(`rustfmt could not be run: ${error.message}`)));
    child.on("close", (code) => {
      if (code === 0) resolvePromise(out);
      else reject(new Error(`rustfmt exited with ${code}: ${err.trim()}`));
    });
    child.stdin.end(source);
  });
}

async function emit(path, contents) {
  if (check) {
    const existing = await readFile(path, "utf8").catch(() => null);
    if (existing !== contents) {
      throw new Error(`${path} is out of date; run node scripts/sync-catalog.mjs`);
    }
    return;
  }
  await writeFile(path, contents);
}
