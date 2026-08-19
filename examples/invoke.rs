//! Invoke a capability against the real backend.
//!
//! Needs a credential, so it prints what it would do and exits cleanly when
//! none is configured — the example is compiled in CI, where there is none.
//!
//! ```sh
//! TINYHUMANS_API_KEY=... cargo run --example invoke -- models.list
//! ```

use portal::{Portal, catalog};
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), portal::Error> {
    let id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "models.list".to_owned());
    let arguments: Value = match std::env::args().nth(2) {
        Some(json) => serde_json::from_str(&json)?,
        None => Value::Null,
    };

    let capability =
        catalog::find(&id).ok_or_else(|| portal::Error::UnknownCapability(id.clone()))?;
    println!("{} -> {}", capability.id, capability.route());

    let portal = Portal::from_env();
    if !portal.settings().is_authenticated() {
        println!("no credential configured; set TINYHUMANS_API_KEY to run this for real");
        return Ok(());
    }

    let result = portal.invoke(&id, &arguments).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
