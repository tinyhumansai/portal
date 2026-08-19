// Curated, human-owned metadata layered on top of the deployed OpenAPI
// contract by `sync-catalog.mjs`. Everything here is editorial: which family a
// route belongs to, which product actually serves it, and the short, memorable
// identifier an agent is expected to type. The route list, parameters, and
// schemas are never written here — they come from the contract.

/**
 * Path prefix -> capability group. Longest prefix wins. The group becomes the
 * first segment of every derived capability id and picks up the provider label
 * and category below.
 */
export const GROUPS = [
  ["/agent-integrations/parallel", "research"],
  ["/agent-integrations/tinyfish", "browse"],
  ["/agent-integrations/composio", "composio"],
  ["/agent-integrations/apify", "apify"],
  ["/agent-integrations/crypto", "crypto"],
  ["/agent-integrations/financial-apis", "finance"],
  ["/agent-integrations/google-places", "places"],
  ["/agent-integrations/media-generation", "media"],
  ["/agent-integrations/file-storage", "files"],
  ["/agent-integrations/recall-calendar", "calendar"],
  ["/agent-integrations/history-rewards", "history"],
  ["/agent-integrations/tenor", "gifs"],
  ["/agent-integrations/twilio", "voice"],
  ["/agent-integrations/pricing", "pricing"],
  ["/openai/v1", "models"],
  ["/medulla/v1", "medulla"],
  ["/orchestration/v1", "orchestration"],
  ["/opencompany", "opencompany"],
  ["/api-keys", "keys"],
  ["/payments", "payments"],
  ["/budgets", "budgets"],
  ["/teams", "teams"],
  ["/auth", "auth"],
  ["/channels", "channels"],
  ["/webhooks", "webhooks"],
  ["/mascots", "mascots"],
  ["/feedback", "feedback"],
  ["/announcements", "announcements"],
  ["/referral", "referral"],
  ["/rewards", "rewards"],
  ["/coupons", "coupons"],
  ["/invite", "invite"],
  ["/r/", "links"],
];

/** Group -> [human provider label, category slug]. */
export const GROUP_META = {
  research: ["Parallel AI", "research"],
  browse: ["TinyFish", "search"],
  composio: ["Composio", "automation"],
  apify: ["Apify", "automation"],
  crypto: ["Crypto routing", "data"],
  finance: ["Alpha Vantage", "data"],
  places: ["Google Places", "data"],
  media: ["GMI (Seedream, Seedance, Veo)", "media"],
  files: ["TinyHumans file storage", "files"],
  calendar: ["Recall.ai", "data"],
  history: ["TinyHumans", "account"],
  gifs: ["Tenor", "media"],
  voice: ["Twilio", "messaging"],
  pricing: ["TinyHumans", "account"],
  models: ["TinyHumans inference gateway", "models"],
  medulla: ["Medulla", "agents"],
  orchestration: ["TinyHumans orchestration", "agents"],
  opencompany: ["OpenCompany", "agents"],
  keys: ["TinyHumans", "account"],
  payments: ["TinyHumans", "account"],
  budgets: ["TinyHumans", "account"],
  teams: ["TinyHumans", "account"],
  auth: ["TinyHumans", "account"],
  channels: ["TinyChannels", "messaging"],
  webhooks: ["TinyHumans", "automation"],
  mascots: ["TinyHumans", "media"],
  feedback: ["TinyHumans", "account"],
  announcements: ["TinyHumans", "account"],
  referral: ["TinyHumans", "account"],
  rewards: ["TinyHumans", "account"],
  coupons: ["TinyHumans", "account"],
  invite: ["TinyHumans", "account"],
  links: ["TinyHumans", "account"],
  health: ["TinyHumans", "account"],
};

/**
 * Short ids for the capabilities an agent reaches for first. The derived id is
 * kept as an alias, so both spellings resolve.
 */
export const ALIASES = {
  "POST /agent-integrations/parallel/search": "search.web",
  "POST /agent-integrations/parallel/chat": "search.chat",
  "POST /agent-integrations/parallel/extract": "search.extract",
  "POST /agent-integrations/parallel/research": "research.start",
  "GET /agent-integrations/parallel/research/{runId}": "research.status",
  "GET /agent-integrations/parallel/research/{runId}/result": "research.result",
  "POST /agent-integrations/tinyfish/search": "search.agentic",
  "POST /agent-integrations/tinyfish/fetch": "search.fetch",
  "GET /openai/v1/models": "models.list",
  "POST /openai/v1/chat/completions": "models.chat",
  "POST /openai/v1/completions": "models.complete",
  "POST /openai/v1/embeddings": "models.embed",
  "POST /openai/v1/audio/speech": "models.speak",
  "POST /openai/v1/audio/transcriptions": "models.transcribe",
  "POST /agent-integrations/media-generation/images": "media.image",
  "POST /agent-integrations/media-generation/videos": "media.video",
  "GET /agent-integrations/media-generation/models": "media.models",
  "GET /agent-integrations/composio/toolkits": "tools.toolkits",
  "GET /agent-integrations/composio/tools": "tools.list",
  "POST /agent-integrations/composio/execute": "tools.execute",
  "GET /agent-integrations/pricing": "pricing.list",
  "GET /payments/credits/balance": "credits.balance",
  "GET /": "health.check",
};

/**
 * Category corrections for routes whose group does not predict how an agent
 * thinks about them. Parallel serves both one-shot search and long-running
 * research; the group can only pick one.
 */
export const CATEGORY_OVERRIDES = {
  "POST /agent-integrations/parallel/search": "search",
  "POST /agent-integrations/parallel/chat": "search",
  "POST /agent-integrations/parallel/extract": "search",
  "POST /agent-integrations/google-places/search": "search",
  "POST /agent-integrations/tenor/search": "search",
};

/**
 * Free-form request bodies for the handful of public routes the deployed
 * contract does not describe. Declared explicitly rather than silently
 * dropped, so the capability still invokes.
 */
export const SUPPLEMENTAL_BODIES = {
  "POST /agent-integrations/tinyfish/search": "Search request forwarded to TinyFish verbatim.",
  "POST /agent-integrations/tinyfish/fetch": "Fetch request forwarded to TinyFish verbatim.",
  "POST /agent-integrations/tinyfish/agent/run": "Agent run request forwarded to TinyFish verbatim.",
};

/** Routes whose successful response is bytes rather than JSON. */
export const BINARY_ROUTES = new Set([
  "GET /agent-integrations/file-storage/files/{fileId}/download",
  "GET /agent-integrations/file-storage/public/{fileId}",
  "GET /mascots/{id}/riv",
  "POST /openai/v1/audio/speech",
]);

/** Routes that take `multipart/form-data`, with the field carrying the file. */
export const MULTIPART_ROUTES = {
  "POST /agent-integrations/file-storage/files": "file",
  "POST /agent-integrations/history-rewards/uploads": "file",
  "POST /openai/v1/audio/transcriptions": "file",
};

/** Routes reachable without a credential. */
export const PUBLIC_ROUTES = new Set([
  "GET /",
  "GET /agent-integrations/file-storage/public/{fileId}",
  "GET /agent-integrations/recall-calendar/oauth-complete",
  "GET /mascots/demo",
  "GET /r/{code}",
]);
