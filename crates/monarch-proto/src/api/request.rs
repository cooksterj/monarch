//! One API call: `{"id":1,"method":"agent.report","params":{…}}`.

use serde::{Deserialize, Serialize};

/// A method call; the answer carries the same `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Chosen by the caller to match the answer.
    pub id: u64,
    /// `namespace.verb`, e.g. `agent.report`.
    pub method: String,
    /// The method's arguments.
    #[serde(default)]
    pub params: serde_json::Value,
}

impl Request {
    /// A call of `method` with `params` serialized to JSON.
    ///
    /// # Errors
    /// `params` cannot be represented as JSON.
    pub fn new(id: u64, method: &str, params: &impl Serialize) -> Result<Self, serde_json::Error> {
        Ok(Self {
            id,
            method: method.into(),
            params: serde_json::to_value(params)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_default_to_null() {
        let request: Request = serde_json::from_str(r#"{"id":4,"method":"x.y"}"#).expect("parse");
        assert_eq!(request.params, serde_json::Value::Null);
        assert!(
            serde_json::from_str::<Request>(r#"{"method":"x.y"}"#).is_err(),
            "no id"
        );
    }

    #[test]
    fn new_serializes_the_params() {
        let request = Request::new(1, "a.b", &[1, 2]).expect("request");
        assert_eq!(
            serde_json::to_string(&request).expect("json"),
            r#"{"id":1,"method":"a.b","params":[1,2]}"#
        );
    }
}
