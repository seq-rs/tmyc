
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
pub use value::Value;

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
