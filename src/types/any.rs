use crate::Scalar;
use serde_value::Value;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

/// Any scalar (For [Apollo Federation](https://www.apollographql.com/docs/apollo-server/federation/introduction))
///
/// The `Any` scalar is used to pass representations of entities from external services into the root `_entities` field for execution.
#[derive(Clone, PartialEq, Eq, Debug, Scalar, Serialize, Deserialize)]
#[graphql(internal, name = "_Any")]
#[serde(transparent)]
pub struct Any(pub Value);

impl Any {
    /// Parse this `Any` value to T.
    pub fn parse_value<T: DeserializeOwned>(self) -> Result<T, serde_value::DeserializerError> {
        self.0.deserialize_into()
    }
}

impl<T: Into<Value>> From<T> for Any {
    fn from(value: T) -> Any {
        Any(value.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_conversion_ok() {
        let value = Value::List(vec![
            Value::Number(1.into()),
            Value::Boolean(true),
            Value::Null,
        ]);
        let expected = Any(value.clone());
        let output: Any = value.into();
        assert_eq!(output, expected);
    }
}
