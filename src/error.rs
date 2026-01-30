use serde_json::Value;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct CliError {
    pub code: u8,
    pub message: String,
    pub details: Option<Value>,
    pub retry_after: Option<u64>,
}

impl CliError {
    pub fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            retry_after: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_retry_after(mut self, retry_after: Option<u64>) -> Self {
        self.retry_after = retry_after;
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;

        // Extract and display GraphQL error messages from details if present
        if let Some(details) = &self.details {
            if let Some(errors) = details.as_array() {
                // details is a GraphQL errors array
                let messages: Vec<&str> = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect();
                if !messages.is_empty() {
                    write!(f, ": {}", messages.join("; "))?;
                }
            } else if let Some(message) = details.get("message").and_then(|m| m.as_str()) {
                // details is an object with a message field
                write!(f, ": {}", message)?;
            } else if let Some(errors) = details.get("errors").and_then(|e| e.as_array()) {
                // details has a nested errors array
                let messages: Vec<&str> = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect();
                if !messages.is_empty() {
                    write!(f, ": {}", messages.join("; "))?;
                }
            }
        }

        Ok(())
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_display_without_details() {
        let err = CliError::new(1, "Simple error");
        assert_eq!(err.to_string(), "Simple error");
    }

    #[test]
    fn test_display_with_graphql_errors_array() {
        let errors = json!([
            {"message": "Field 'foo' not found"},
            {"message": "Invalid query syntax"}
        ]);
        let err = CliError::new(1, "GraphQL error").with_details(errors);
        assert_eq!(
            err.to_string(),
            "GraphQL error: Field 'foo' not found; Invalid query syntax"
        );
    }

    #[test]
    fn test_display_with_single_graphql_error() {
        let errors = json!([{"message": "Entity not found"}]);
        let err = CliError::new(1, "GraphQL error").with_details(errors);
        assert_eq!(err.to_string(), "GraphQL error: Entity not found");
    }

    #[test]
    fn test_display_with_object_message() {
        let details = json!({"message": "Rate limit exceeded", "code": 429});
        let err = CliError::new(4, "API error").with_details(details);
        assert_eq!(err.to_string(), "API error: Rate limit exceeded");
    }

    #[test]
    fn test_display_with_nested_errors_array() {
        let details = json!({
            "errors": [
                {"message": "Permission denied"},
                {"message": "Insufficient scope"}
            ]
        });
        let err = CliError::new(3, "Auth error").with_details(details);
        assert_eq!(
            err.to_string(),
            "Auth error: Permission denied; Insufficient scope"
        );
    }

    #[test]
    fn test_display_with_empty_errors_array() {
        let errors = json!([]);
        let err = CliError::new(1, "GraphQL error").with_details(errors);
        assert_eq!(err.to_string(), "GraphQL error");
    }

    #[test]
    fn test_display_with_errors_missing_message() {
        let errors = json!([{"code": 123}, {"extensions": {}}]);
        let err = CliError::new(1, "GraphQL error").with_details(errors);
        assert_eq!(err.to_string(), "GraphQL error");
    }
}
