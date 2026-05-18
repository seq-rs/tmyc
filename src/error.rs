#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.col) {
            (None, None) => write!(f, "Parsing error: {}", self.msg),
            (Some(l), None) => write!(f, "Parsing error on line {}: {}", l, self.msg),
            (None, Some(p)) => write!(f, "Parsing error on pos {}: {}", p, self.msg),
            (Some(l), Some(p)) => write!(f, "Parsing error on line {}, pos {}: {}", l, p, self.msg),
        }
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error {
            msg: msg.to_string(),
            line: None,
            col: None,
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error {
            msg: msg.to_string(),
            line: None,
            col: None,
        }
    }
}
