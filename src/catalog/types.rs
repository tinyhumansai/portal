//! The vocabulary the capability table is written in.
//!
//! Every type here is `'static` data: the catalog is compiled in, not loaded,
//! so a capability lookup cannot fail at runtime for want of a file and the
//! CLI, the MCP server, and the library all agree on the same surface.

use serde::Serialize;
use serde_json::{Map, Value, json};

/// The families a capability can belong to.
///
/// Categories exist for the agent's benefit: they are the first cut an agent
/// makes when it knows what it wants to do but not what the route is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Web search, page fetching, and place or media lookup.
    Search,
    /// Long-running deep research, dataset building, and enrichment.
    Research,
    /// Chat, completion, embedding, speech, and transcription models.
    Models,
    /// Image, video, and animated-asset generation.
    Media,
    /// Market, financial, location, calendar, and chain data.
    Data,
    /// Third-party tool execution, scraping actors, triggers, and webhooks.
    Automation,
    /// File storage, upload, download, and sharing.
    Files,
    /// Chat channels and outbound voice.
    Messaging,
    /// Agent sessions, workflows, tasks, and hosted companies.
    Agents,
    /// Identity, teams, credits, billing, and quotas.
    Account,
}

impl Category {
    /// Every category, in the order they are worth reading.
    pub const ALL: &'static [Self] = &[
        Self::Search,
        Self::Research,
        Self::Models,
        Self::Media,
        Self::Data,
        Self::Automation,
        Self::Files,
        Self::Messaging,
        Self::Agents,
        Self::Account,
    ];

    /// The lowercase identifier used on the command line and the wire.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Research => "research",
            Self::Models => "models",
            Self::Media => "media",
            Self::Data => "data",
            Self::Automation => "automation",
            Self::Files => "files",
            Self::Messaging => "messaging",
            Self::Agents => "agents",
            Self::Account => "account",
        }
    }

    /// A one-line description of what lives in the category.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Search => "web search, page fetching, places, and gifs",
            Self::Research => "deep research runs, datasets, and enrichment",
            Self::Models => "chat, completion, embedding, speech, and transcription",
            Self::Media => "image, video, and animated asset generation",
            Self::Data => "market, location, calendar, and on-chain data",
            Self::Automation => "third-party tools, scrapers, triggers, and webhooks",
            Self::Files => "file storage, upload, download, and sharing",
            Self::Messaging => "chat channels and outbound voice calls",
            Self::Agents => "agent sessions, workflows, tasks, and companies",
            Self::Account => "identity, teams, credits, billing, and quotas",
        }
    }

    /// Parse a category from its [`slug`](Self::slug), case-insensitively.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        Self::ALL.iter().copied().find(|c| c.slug() == value)
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// The HTTP method a capability is invoked with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// The uppercase HTTP spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    /// Whether the capability only reads state.
    ///
    /// Agents use this to decide what is safe to call speculatively; the CLI
    /// and MCP server use it to decide what needs confirmation.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self, Self::Get)
    }
}

impl From<Method> for reqwest::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::Get => Self::GET,
            Method::Post => Self::POST,
            Method::Put => Self::PUT,
            Method::Patch => Self::PATCH,
            Method::Delete => Self::DELETE,
        }
    }
}

/// Where an argument is placed in the outgoing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamIn {
    /// Substituted into the path template.
    Path,
    /// Appended to the query string.
    Query,
    /// Carried in the request body.
    Body,
}

/// The JSON type an argument must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    /// A JSON string.
    String,
    /// A whole number.
    Integer,
    /// Any number.
    Number,
    /// `true` or `false`.
    Boolean,
    /// A JSON array.
    Array,
    /// A JSON object.
    Object,
    /// A path to a local file, read and uploaded by Portal.
    File,
    /// Any JSON value; the contract does not constrain it.
    Any,
}

impl ParamKind {
    /// The JSON Schema `type` for the kind, if it constrains one.
    #[must_use]
    pub const fn json_type(self) -> Option<&'static str> {
        match self {
            Self::String | Self::File => Some("string"),
            Self::Integer => Some("integer"),
            Self::Number => Some("number"),
            Self::Boolean => Some("boolean"),
            Self::Array => Some("array"),
            Self::Object => Some("object"),
            Self::Any => None,
        }
    }

    /// How the kind reads in an error message: "a string", "an integer".
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::String => "a string",
            Self::Integer => "an integer",
            Self::Number => "a number",
            Self::Boolean => "a boolean",
            Self::Array => "an array",
            Self::Object => "an object",
            Self::File => "a path to a local file",
            Self::Any => "any json value",
        }
    }

    /// Whether `value` satisfies the kind.
    #[must_use]
    pub fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String | Self::File => value.is_string(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Number => value.is_number(),
            Self::Boolean => value.is_boolean(),
            Self::Array => value.is_array(),
            Self::Object => value.is_object(),
            Self::Any => true,
        }
    }
}

/// How the request body is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    /// No body is sent.
    None,
    /// Body parameters are collected into one JSON object.
    Json,
    /// Body parameters become `multipart/form-data` fields.
    Multipart,
}

/// Whether the backend wraps the response in a `{success, data}` envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Envelope {
    /// Unwrap the envelope and return `data`.
    Unwrap,
    /// Return the body exactly as the backend sent it.
    Raw,
}

/// The shape of a successful response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseKind {
    /// A JSON value.
    Json,
    /// Raw bytes, such as an audio file or a stored document.
    Binary,
}

/// Whether a credential is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Auth {
    /// A bearer token or API key must be configured.
    Required,
    /// Callable without a credential.
    Public,
}

/// One argument a capability accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Param {
    /// The `snake_case` name an agent supplies.
    pub name: &'static str,
    /// The name the backend expects, which may be `camelCase`.
    pub wire_name: &'static str,
    /// Where the value ends up in the request.
    pub location: ParamIn,
    /// The JSON type the value must have.
    pub kind: ParamKind,
    /// Whether the call is rejected when the value is absent.
    pub required: bool,
    /// What the value means, taken from the backend contract.
    pub description: &'static str,
}

impl Param {
    /// Whether `name` refers to this parameter under either spelling.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        self.name == name || self.wire_name == name
    }
}

/// One callable backend operation.
///
/// A capability is the unit everything in this crate is built around: the CLI
/// invokes one, the MCP server exposes the catalog of them, and the library
/// dispatches by identifier. Its data comes from the deployed contract, so the
/// summary and parameter documentation are the backend's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capability {
    /// The stable identifier callers invoke, such as `search.web`.
    pub id: &'static str,
    /// Other identifiers that resolve to this capability.
    pub aliases: &'static [&'static str],
    /// The family the capability belongs to.
    pub category: Category,
    /// The catalog group, which is the first segment of the derived id.
    pub group: &'static str,
    /// The product actually serving the call.
    pub provider: &'static str,
    /// One line describing what the capability does.
    pub summary: &'static str,
    /// The HTTP method.
    pub method: Method,
    /// The backend path template, with `{placeholder}` path parameters.
    pub path: &'static str,
    /// Every argument the capability accepts.
    pub params: &'static [Param],
    /// How the request body is encoded.
    pub body: BodyKind,
    /// Whether the response envelope is unwrapped.
    pub envelope: Envelope,
    /// Whether the response is JSON or bytes.
    pub response: ResponseKind,
    /// Whether a credential is required.
    pub auth: Auth,
}

impl Capability {
    /// Whether `name` is this capability's id or one of its aliases.
    #[must_use]
    pub fn answers_to(&self, name: &str) -> bool {
        self.id == name || self.aliases.contains(&name)
    }

    /// Look up one parameter by either of its names.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&'static Param> {
        self.params.iter().find(|param| param.matches(name))
    }

    /// The parameters that must be supplied.
    pub fn required_params(&self) -> impl Iterator<Item = &'static Param> {
        self.params.iter().filter(|param| param.required)
    }

    /// The route as it appears in the backend contract, such as `GET /teams`.
    #[must_use]
    pub fn route(&self) -> String {
        format!("{} {}", self.method.as_str(), self.path)
    }

    /// Whether the capability returns bytes rather than JSON.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        matches!(self.response, ResponseKind::Binary)
    }

    /// Whether a credential must be configured before invoking.
    #[must_use]
    pub fn needs_auth(&self) -> bool {
        matches!(self.auth, Auth::Required)
    }

    /// Whether the capability uploads a local file.
    #[must_use]
    pub fn is_upload(&self) -> bool {
        matches!(self.body, BodyKind::Multipart)
    }

    /// A JSON Schema describing the capability's arguments.
    ///
    /// This is what the MCP server advertises as a tool's `inputSchema` and
    /// what `portal describe --json` prints, so an agent reads one shape
    /// whichever door it came through.
    #[must_use]
    pub fn input_schema(&self) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for param in self.params {
            let mut entry = Map::new();
            if let Some(json_type) = param.kind.json_type() {
                entry.insert("type".to_owned(), Value::String(json_type.to_owned()));
            }
            let mut description = param.description.to_owned();
            if param.kind == ParamKind::File {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str("Give a path to a file on this machine.");
            }
            if !description.is_empty() {
                entry.insert("description".to_owned(), Value::String(description));
            }
            properties.insert(param.name.to_owned(), Value::Object(entry));
            if param.required {
                required.push(Value::String(param.name.to_owned()));
            }
        }
        json!({
            "type": "object",
            "properties": Value::Object(properties),
            "required": Value::Array(required),
            "additionalProperties": false,
        })
    }
}
