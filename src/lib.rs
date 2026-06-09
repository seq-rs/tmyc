//! # tmyc — YAML data format implementation for serde
//!
//! A YAML 1.2 best-effort parser and emmitter with the goals of:
//!
//! - Filling the gap after archival of `serde-yaml`
//! - Best effort compliance to support common docker-compose, kubernetes resource and configuration files
//! - Trustworthy, least-dependencies implementation to avoid the trust issues surrounding other similarly motivated replacement crates dismissed for suspicious, inexplicable dependencies and code
//!
//! ## Quick start
//!
//! ```
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Service {
//!     name: String,
//!     port: u16,
//! }
//!
//! let yaml = "name: web\nport: 8080\n";
//! let svc: Service = tmyc::from_str(yaml).unwrap();
//! assert_eq!(svc, Service { name: "web".into(), port: 8080 });
//!
//! let back = tmyc::to_string(&svc).unwrap();
//! assert_eq!(back, yaml);
//! ```
//!
//! ## Entry points
//!
//! | Function | Purpose |
//! |---|---|
//! | [`from_str`]    | Deserialize a single document into `T: DeserializeOwned`. |
//! | [`from_value`]  | Deserialize from a pre-parsed [`BorrowedValue`], supporting zero-copy borrows. |
//! | [`to_string`]   | Serialize `T: Serialize` to a YAML string. |
//! | [`to_value`]    | Serialize `T: Serialize` to a [`BorrowedValue`] (for inspection or post-processing). |
//! | [`Parser`]      | Manual parsing API. Use [`Parser::parse_all`] for multi-document streams. |
//!
//! ## Why `DeserializeOwned` for [`from_str`]?
//!
//! [`from_str`] builds an intermediate [`BorrowedValue`] that lives only for the call.
//! If your target type held borrowed `&str` fields they'd reference a BorrowedValue
//! that's already been dropped — unsound. The [`serde::de::DeserializeOwned`]
//! bound rules out borrowed fields at compile time.
//!
//! To get the zero-copy payoff (struct fields that are `&str` slices of the
//! input), keep a [`BorrowedValue`] alive yourself and use [`from_value`]:
//!
//! ```
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Borrowed<'a> { name: &'a str }
//!
//! let src = "name: hello\n";
//! let value = tmyc::Parser::new(src).parse().unwrap();
//! let b: Borrowed<'_> = tmyc::from_value(&value).unwrap();
//! assert_eq!(b.name, "hello");
//! ```
//!
//! ## Multi-document streams
//!
//! Files written by tools like `kubectl get all -o yaml` contain multiple
//! documents separated by `---`. Use [`Parser::parse_all`]:
//!
//! ```
//! let stream = "\
//! ---
//! kind: Pod
//! ---
//! kind: Service
//! ";
//! let docs = tmyc::Parser::new(stream).parse_all().unwrap();
//! assert_eq!(docs.len(), 2);
//! ```
//!
//! ## YAML feature coverage
//!
//! Per YAML 1.2:
//! - Block and flow scalars (literal `|`, folded `>`, plain, single/double quoted)
//! - Block and flow containers (sequences and mappings)
//! - Standard tags (`!!str`, `!!int`, `!!float`, `!!bool`, `!!null`) with coercion
//! - Custom tags preserved via [`BorrowedValue::Tagged`]
//! - Anchors (`&name`) and aliases (`*name`), document-scoped per spec
//! - Multi-document streams (`---`/`...`)
//! - UTF-8 BOM and leading directives (`%YAML`/`%TAG`) tolerated
//!
//! Beyond strict YAML 1.2:
//! - Merge keys (`<<: *base`) resolved automatically — heavy in docker-compose.
//!
//! Not implemented (rare in target ecosystems):
//! - Explicit complex keys (`? key: value`)
//! - Strict `%YAML` version enforcement
//! - `%TAG` handle substitution
//!
//! ## Design principles
//!
//! - **Spec-correct parser, pragmatic emitter.** Roundtrip-equal in *data*, not
//!   necessarily byte-identical in *presentation*.
//! - **Zero-copy via `Cow<'a, str>`.** Plain and unescaped quoted scalars are
//!   borrowed slices of the input; allocation only when escapes or folds force it.
//! - **Lossless scalar resolution.** Plain `42` → `Int/UInt(42)`; quoted `"42"`
//!   stays `String("42")`.

mod de;
mod emitter;
mod error;
mod parser;
mod patterns;
mod ser;
mod value;

pub use de::{from_str, from_value};
pub use error::{Error, Result};
pub use parser::Parser;
pub use ser::to_value;
pub use value::BorrowedValue;

/// Serialize `T` to a YAML string.
///
/// Composes [`to_value`] with the internal emitter. For inspection or
/// transformation of the intermediate representation, use [`to_value`] directly.
///
/// # Example
///
/// ```
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Greet { hello: String }
///
/// let g = Greet { hello: "world".to_string() };
/// assert_eq!(tmyc::to_string(&g).unwrap(), "hello: world\n");
/// ```
pub fn to_string<T: ?Sized + serde::Serialize>(v: &T) -> Result<String> {
    let value = ser::to_value(v)?;
    emitter::emit(&value)
}
