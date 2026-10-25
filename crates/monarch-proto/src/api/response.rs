//! The answer to one [`super::Request`].

use serde::{Deserialize, Serialize};

use super::RpcError;

/// `{"id":1,"result":…}` or `{"id":1,"error":{…}}`; `id` is `null` when
/// the request could not be read at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The request's id.
    pub id: Option<u64>,
    /// What the method returned, on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Why it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    /// Success with `result`.
    #[must_use]
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    /// Failure of request `id` (`None` if unreadable).
    #[must_use]
    pub fn err(id: Option<u64>, error: RpcError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_and_failure_shapes() {
        let ok = Response::ok(3, serde_json::json!({}));
        assert_eq!(
            serde_json::to_string(&ok).expect("json"),
            r#"{"id":3,"result":{}}"#
        );
        let bad = Response::err(None, RpcError::new(RpcError::PARSE, "not json"));
        assert_eq!(
            serde_json::to_string(&bad).expect("json"),
            r#"{"id":null,"error":{"code":-32700,"message":"not json"}}"#
        );
    }
}
