use super::prelude::*;

#[derive(Debug, Error)]
#[error("{message}")]
pub(crate) struct AppError {
    pub(crate) code: String,
    pub(crate) message: String,
    #[source]
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
    pub(crate) details: Value,
    pub(crate) suggestions: Vec<String>,
}

impl AppError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            source: None,
            details: Value::Object(Map::new()),
            suggestions: Vec::new(),
        }
    }

    pub(crate) fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub(crate) fn with_suggestions(mut self, suggestions: &[&str]) -> Self {
        self.suggestions = suggestions.iter().map(|item| (*item).to_string()).collect();
        self
    }

    pub(crate) fn from_io(code: &str, message: impl Into<String>, source: io::Error) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            source: Some(Box::new(source)),
            details: Value::Object(Map::new()),
            suggestions: Vec::new(),
        }
    }
}
