# The Missing YAML Crate

A serde YAML implementation to replace the crate `serde-yaml`, which was
archived.

## Motivation

Since the crate was archived, every alternative was either unmaintained fast,
had questionable code/dependencies or had other issues. To avoid investigating
after YAML crates, I decided to make this one with the goals of:

- Best effort compliance: YAML 1.2 specs are targeted for compliance, but with
  some specific stuff like directives parsed but not functional.
- Zero-copy: parsing is zero-copy, with the only cloned values being anchored
  values.
- Have some yaml crate I can use for my projects without issues.

## Usage

Like other serde crates, define a data structure you want to parse into:

```rust
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
```

then to parse a file into this data structure, for example:

```yaml
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
```

Finally, parse the file:

```rust
fn main() -> tmyc::Result<()> {
    // Parse all docs, then deserialize each into a typed Resource.
    let docs = tmyc::Parser::new(stream).parse_all()?;
    println!("Stream contains {} documents", docs.len());

    for doc in &docs {
        let r: Resource = tmyc::from_value(doc)?;
        println!("  {} {} ({})", r.api_version, r.kind, r.metadata.name);
    }

    Ok(())
}
```

## Dependencies

`serde`: not sure it needs explaining, `serde` defines the traits we are implementing.

## Licence

MIT or Apache-2.0

## AI Attribution

AI was used for bughunting during development, test writing and documentation.
