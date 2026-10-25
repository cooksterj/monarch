//! Why an API call failed: JSON-RPC style code and message.

use serde::{Deserialize, Serialize};

/// A failed call's code and human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    /// One of the constants below.
    pub code: i32,
    /// What went wrong.
    pub message: String,
}

impl RpcError {
    /// The line is not a request.
    pub const PARSE: i32 = -32700;
    /// No such method.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// The params don't fit the method.
    pub const INVALID_PARAMS: i32 = -32602;
    /// The pane named does not exist in this session.
    pub const NO_SUCH_PANE: i32 = -32001;
    /// The method was understood but could not be done (the message says
    /// why).
    pub const FAILED: i32 = -32002;

    /// An error with `code` and `message`.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let error = RpcError::new(RpcError::METHOD_NOT_FOUND, "no x.y");
        assert_eq!(
            serde_json::to_string(&error).expect("json"),
            r#"{"code":-32601,"message":"no x.y"}"#
        );
        assert_ne!(error, RpcError::new(RpcError::PARSE, "no x.y"));
    }
}
