use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ImportError(String);

impl ImportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ImportError {}

pub type Result<T> = std::result::Result<T, ImportError>;
