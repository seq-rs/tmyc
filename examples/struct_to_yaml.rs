//! Round-trip a derived struct through YAML.
//!
//! Run with: `cargo run --example struct_to_yaml`

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug)]
struct Service {
    image: String,
    ports: Vec<String>,
    #[serde(default)]
    environment: BTreeMap<String, String>,
    #[serde(default)]
    restart: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Compose {
    version: String,
    services: BTreeMap<String, Service>,
}

fn main() -> yaml0::Result<()> {
    // Build a small compose-like structure in code
    let mut services = BTreeMap::new();
    services.insert(
        "web".into(),
        Service {
            image: "nginx:1.25".into(),
            ports: vec!["80:80".into(), "443:443".into()],
            environment: BTreeMap::new(),
            restart: Some("always".into()),
        },
    );

    let mut api_env = BTreeMap::new();
    api_env.insert("LOG_LEVEL".into(), "info".into());
    services.insert(
        "api".into(),
        Service {
            image: "api:latest".into(),
            ports: vec!["3000:3000".into()],
            environment: api_env,
            restart: Some("on-failure".into()),
        },
    );

    let compose = Compose { version: "3.8".into(), services };

    // Struct → YAML
    let yaml = yaml0::to_string(&compose)?;
    println!("=== Serialised YAML ===\n{yaml}");

    // YAML → Struct
    let parsed: Compose = yaml0::from_str(&yaml)?;
    println!("=== Re-parsed struct ===");
    println!("version: {}", parsed.version);
    for (name, svc) in &parsed.services {
        println!("  {name}: {} ({} ports)", svc.image, svc.ports.len());
    }

    Ok(())
}
