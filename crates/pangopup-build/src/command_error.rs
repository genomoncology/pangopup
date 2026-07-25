//! Shared serialized command failure used by artifact builders and adapters.

use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, Serialize)]
pub struct CommandError {
    pub status: &'static str,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: "error",
            code,
            message: message.into(),
            details: None,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}
