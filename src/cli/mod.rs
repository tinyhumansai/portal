//! The `portal` command line.
//!
//! The CLI is the same catalog the library and the MCP server use, with a
//! terminal in front of it. Its shape follows how an agent actually works:
//!
//! ```sh
//! portal catalog                     # what families exist
//! portal search "transcribe audio"   # find the capability
//! portal describe models.transcribe  # read its arguments
//! portal call models.transcribe --arg file=./call.mp3
//! ```
//!
//! Convenience commands (`portal web`, `portal chat`, `portal models`) are
//! shorthands for the same `call`, and `portal mcp` serves the whole thing over
//! the Model Context Protocol.

pub mod render;

use std::io::Write;

use clap::{Parser, Subcommand};
use serde_json::{Map, Value, json};

use crate::catalog::{self, Capability, Category, ParamKind};
use crate::client::Portal;
use crate::config::Settings;
use crate::error::{Error, Result};
use crate::mcp;

/// One agent-friendly door to every `TinyHumans` provider, model, and search
/// engine.
#[derive(Debug, Parser)]
#[command(name = "portal", version, about, long_about = None)]
pub struct Cli {
    /// Backend origin to call. Defaults to `TINYHUMANS_BASE_URL`.
    #[arg(long, global = true)]
    pub base_url: Option<String>,

    /// Print machine-readable JSON instead of a human listing.
    #[arg(long, global = true)]
    pub json: bool,

    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The `portal` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List capability categories, or the capabilities in one.
    Catalog {
        /// Restrict to one category, such as `search` or `models`.
        #[arg(long)]
        category: Option<String>,
        /// Restrict to one group, such as `medulla` or `files`.
        #[arg(long)]
        group: Option<String>,
    },
    /// Find capabilities in plain words.
    Search {
        /// The words to search for.
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
        /// Maximum matches to print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show one capability's arguments, route, and example invocation.
    Describe {
        /// A capability identifier or alias.
        capability: String,
    },
    /// Invoke a capability.
    Call {
        /// A capability identifier or alias.
        capability: String,
        /// An argument, as `name=value`. Repeatable.
        #[arg(long = "arg", value_name = "NAME=VALUE")]
        args: Vec<String>,
        /// The whole argument object as JSON, merged under `--arg`.
        #[arg(long, value_name = "JSON")]
        args_json: Option<String>,
        /// Write a byte response to this path instead of printing it.
        #[arg(long, value_name = "PATH")]
        out: Option<String>,
        /// Print the request that would be sent and stop.
        #[arg(long)]
        dry_run: bool,
    },
    /// List the models available for inference.
    Models,
    /// Send one prompt to a chat model.
    Chat {
        /// The prompt to send.
        #[arg(required = true, num_args = 1..)]
        prompt: Vec<String>,
        /// The model to use.
        #[arg(long, default_value = "tiny-1")]
        model: String,
    },
    /// Search the web.
    Web {
        /// The question to answer.
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
    },
    /// Call a public backend route the catalog does not cover.
    Raw {
        /// The HTTP method.
        method: String,
        /// The path, starting with `/`.
        path: String,
        /// A JSON request body.
        #[arg(long, value_name = "JSON")]
        body: Option<String>,
    },
    /// Report the configured backend, credential, and reachability.
    Status,
    /// Serve the catalog over the Model Context Protocol on stdio.
    Mcp,
    /// Print the Agent Skill that teaches an agent to use Portal.
    Skill,
}

/// The Agent Skill, embedded so `portal skill` works from an installed binary.
const SKILL: &str = include_str!("../../skills/portal/SKILL.md");

/// Parse the process arguments and run the requested command.
///
/// # Errors
///
/// Returns whatever the chosen command returns; see [`execute`].
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    execute(cli, &mut stdout).await
}

/// Run one already-parsed command, writing output to `out`.
///
/// # Errors
///
/// Returns [`Error::UnknownCapability`] or an argument error for a bad call,
/// [`Error::Backend`] when the backend rejects it, and [`Error::Io`] when the
/// output stream or an output file cannot be written.
pub async fn execute(cli: Cli, out: &mut impl Write) -> Result<()> {
    let mut settings = Settings::from_env();
    if let Some(base_url) = &cli.base_url {
        settings = Settings::new(base_url)
            .with_api_key(settings.api_key().map(str::to_owned))
            .with_token(settings.token().map(str::to_owned));
    }
    execute_with(&Portal::new(settings), cli.command, cli.json, out).await
}

/// Run one command against an already-built client.
///
/// The credential never appears on the command line, so this is also how a
/// caller — an embedding program, or a test against a local server — supplies
/// one.
///
/// # Errors
///
/// See [`execute`].
pub async fn execute_with(
    portal: &Portal,
    command: Command,
    as_json: bool,
    out: &mut impl Write,
) -> Result<()> {
    match command {
        Command::Catalog { category, group } => {
            catalog_command(as_json, category.as_deref(), group.as_deref(), out)
        }
        Command::Search { query, limit } => {
            let hits: Vec<&Capability> = catalog::search(&query.join(" "))
                .into_iter()
                .take(limit)
                .collect();
            emit_capabilities(as_json, &hits, out)
        }
        Command::Describe { capability } => {
            let capability = resolve(&capability)?;
            if as_json {
                writeln!(out, "{}", render::json(&mcp::describe_value(capability)))?;
            } else {
                write!(out, "{}", render::describe(capability))?;
            }
            Ok(())
        }
        Command::Call {
            capability,
            args,
            args_json,
            out: destination,
            dry_run,
        } => {
            call_command(
                portal,
                &capability,
                &args,
                args_json.as_deref(),
                destination.as_deref(),
                dry_run,
                out,
            )
            .await
        }
        Command::Models => emit_call(portal, "models.list", &Value::Null, out).await,
        Command::Chat { prompt, model } => {
            let arguments = json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt.join(" ") }],
            });
            emit_call(portal, "models.chat", &arguments, out).await
        }
        Command::Web { query } => {
            let objective = query.join(" ");
            let arguments = json!({ "objective": objective, "search_queries": [objective] });
            emit_call(portal, "search.web", &arguments, out).await
        }
        Command::Raw { method, path, body } => {
            let method = parse_method(&method)?;
            let body = body.map(|body| serde_json::from_str(&body)).transpose()?;
            let result = portal.raw(method, &path, body.as_ref()).await?;
            writeln!(out, "{}", render::json(&result))?;
            Ok(())
        }
        Command::Status => status_command(portal, as_json, out).await,
        Command::Mcp => mcp::Server::new(portal.clone()).serve().await,
        Command::Skill => {
            write!(out, "{SKILL}")?;
            Ok(())
        }
    }
}

/// Invoke a capability and print its JSON result.
async fn emit_call(
    portal: &Portal,
    id: &str,
    arguments: &Value,
    out: &mut impl Write,
) -> Result<()> {
    let result = portal.invoke(id, arguments).await?;
    writeln!(out, "{}", render::json(&result))?;
    Ok(())
}

/// `portal status`: configuration first, then whether the backend answers.
async fn status_command(portal: &Portal, as_json: bool, out: &mut impl Write) -> Result<()> {
    let settings = portal.settings();
    let backend = match portal.invoke("health.check", &Value::Null).await {
        Ok(_) => "reachable".to_owned(),
        Err(error) => format!("unreachable ({error})"),
    };
    if as_json {
        writeln!(
            out,
            "{}",
            render::json(&json!({
                "version": crate::VERSION,
                "base_url": settings.base_url(),
                "credential": settings.credential_kind(),
                "capabilities": catalog::len(),
                "backend": backend,
            }))
        )?;
    } else {
        writeln!(out, "portal {}", crate::VERSION)?;
        writeln!(out, "  backend      {}", settings.base_url())?;
        writeln!(out, "  credential   {}", settings.credential_kind())?;
        writeln!(out, "  capabilities {}", catalog::len())?;
        writeln!(out, "  reachability {backend}")?;
    }
    Ok(())
}

/// `portal catalog`, with or without a filter.
fn catalog_command(
    as_json: bool,
    category: Option<&str>,
    group: Option<&str>,
    out: &mut impl Write,
) -> Result<()> {
    if category.is_none() && group.is_none() && !as_json {
        write!(out, "{}", render::categories())?;
        return Ok(());
    }
    let category = match category.map(Category::parse) {
        Some(None) => {
            return Err(Error::InvalidArgument {
                param: "category".to_owned(),
                expected: "one of the catalog categories".to_owned(),
            });
        }
        Some(Some(category)) => Some(category),
        None => None,
    };
    let hits: Vec<&Capability> = catalog::all()
        .iter()
        .filter(|capability| category.is_none_or(|wanted| capability.category == wanted))
        .filter(|capability| group.is_none_or(|wanted| capability.group == wanted))
        .collect();
    emit_capabilities(as_json, &hits, out)
}

/// `portal call`, including its dry run and byte-output forms.
async fn call_command(
    portal: &Portal,
    id: &str,
    args: &[String],
    args_json: Option<&str>,
    destination: Option<&str>,
    dry_run: bool,
    out: &mut impl Write,
) -> Result<()> {
    let capability = resolve(id)?;
    let arguments = collect_arguments(capability, args, args_json)?;

    if dry_run {
        let request = crate::invoke::plan(capability, &arguments)?;
        writeln!(out, "{}", request.describe())?;
        if let Some(body) = &request.body {
            writeln!(out, "{}", render::json(body))?;
        }
        return Ok(());
    }
    if let Some(destination) = destination {
        let bytes = portal.invoke_bytes(capability.id, &arguments).await?;
        std::fs::write(destination, &bytes)?;
        writeln!(out, "wrote {} bytes to {destination}", bytes.len())?;
        return Ok(());
    }
    let result = portal.invoke(capability.id, &arguments).await?;
    writeln!(out, "{}", render::json(&result))?;
    Ok(())
}

/// Look up a capability, or fail with the closest matches attached.
///
/// A mistyped identifier is the most common way to get here, so the suggestion
/// is by shared prefix rather than by search: `models.chatt` finds
/// `models.chat`, which a word search never would.
fn resolve(id: &str) -> Result<&'static Capability> {
    catalog::find(id).ok_or_else(|| {
        let mut ranked: Vec<(usize, &'static str)> = catalog::all()
            .iter()
            .map(|capability| (shared_prefix(id, capability.id), capability.id))
            .filter(|(shared, _)| *shared >= 3)
            .collect();
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
        let suggestions: Vec<&str> = ranked.into_iter().take(3).map(|(_, id)| id).collect();
        if suggestions.is_empty() {
            Error::UnknownCapability(id.to_owned())
        } else {
            Error::UnknownCapability(format!("{id} (did you mean {}?)", suggestions.join(", ")))
        }
    })
}

/// How many leading bytes two identifiers share.
fn shared_prefix(left: &str, right: &str) -> usize {
    left.bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count()
}

/// Merge `--args-json` and repeated `--arg name=value` into one object.
///
/// A `--arg` value is read as JSON when the capability expects a structured
/// type, and taken literally when it expects a string — so `--arg model=tiny-1`
/// does not have to be quoted and `--arg n=3` still arrives as a number.
fn collect_arguments(
    capability: &'static Capability,
    args: &[String],
    args_json: Option<&str>,
) -> Result<Value> {
    let mut arguments = match args_json {
        Some(text) => match serde_json::from_str(text)? {
            Value::Object(map) => map,
            other => {
                return Err(Error::InvalidArguments {
                    got: match other {
                        Value::Null => "null",
                        Value::Bool(_) => "boolean",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                    },
                });
            }
        },
        None => Map::new(),
    };
    for pair in args {
        let (name, value) = pair.split_once('=').ok_or_else(|| Error::InvalidArgument {
            param: pair.clone(),
            expected: "written as name=value".to_owned(),
        })?;
        let literal = capability
            .param(name)
            .is_some_and(|param| matches!(param.kind, ParamKind::String | ParamKind::File));
        let value = if literal {
            Value::String(value.to_owned())
        } else {
            serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
        };
        arguments.insert(name.to_owned(), value);
    }
    Ok(Value::Object(arguments))
}

/// Print capabilities as a listing or as JSON.
fn emit_capabilities(as_json: bool, hits: &[&Capability], out: &mut impl Write) -> Result<()> {
    if as_json {
        writeln!(out, "{}", render::json(&json!(hits)))?;
    } else {
        write!(out, "{}", render::listing(hits))?;
    }
    Ok(())
}

/// Parse an HTTP method for `portal raw`.
fn parse_method(value: &str) -> Result<catalog::Method> {
    match value.to_ascii_uppercase().as_str() {
        "GET" => Ok(catalog::Method::Get),
        "POST" => Ok(catalog::Method::Post),
        "PUT" => Ok(catalog::Method::Put),
        "PATCH" => Ok(catalog::Method::Patch),
        "DELETE" => Ok(catalog::Method::Delete),
        _ => Err(Error::InvalidArgument {
            param: "method".to_owned(),
            expected: "one of GET, POST, PUT, PATCH, DELETE".to_owned(),
        }),
    }
}

#[cfg(test)]
mod test;
