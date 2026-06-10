//! Parse a multi-document YAML stream like `kubectl get all -o yaml`.
//!
//! Run with: `cargo run --example parse_kubectl_stream`

use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Resource {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
    metadata: Metadata,
}

#[derive(Deserialize, Debug)]
struct Metadata {
    name: String,
}

fn main() -> yaml0::Result<()> {
    let stream = "\
---
apiVersion: v1
kind: Pod
metadata:
  name: web-7d8f
spec:
  containers:
    - image: nginx
---
apiVersion: v1
kind: Service
metadata:
  name: web-svc
spec:
  ports:
    - port: 80
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web-deploy
";

    // Parse all docs, then deserialize each into a typed Resource.
    let docs = yaml0::Parser::new(stream).parse_all()?;
    println!("Stream contains {} documents", docs.len());

    for doc in &docs {
        let r: Resource = yaml0::from_value(doc)?;
        println!("  {} {} ({})", r.api_version, r.kind, r.metadata.name);
    }

    Ok(())
}
