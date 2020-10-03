use crate::{Scalar, ScalarType};
use num_traits::Num;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// A numeric value represented by a string.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Serialize, Deserialize, Scalar)]
#[graphql(internal)]
#[serde(transparent)]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "string_number")))]
pub struct StringNumber<T: Num + Display>(pub T);

#[cfg(test)]
mod test {
    use crate::*;

    #[async_std::test]
    async fn test_string_number() {
        struct Query;

        #[Object(internal)]
        impl Query {
            async fn value(&self, n: StringNumber<i32>) -> StringNumber<i32> {
                n
            }
        }

        let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
        assert_eq!(
            schema
                .execute(
                    r#"{
                    value1: value(n: "100")
                    value2: value(n: "-100")
                    value3: value(n: "0")
                    value4: value(n: "1")
                }"#
                )
                .await
                .into_result()
                .unwrap()
                .data,
            serde_json::json!({
                "value1": "100",
                "value2": "-100",
                "value3": "0",
                "value4": "1",
            })
        );
    }
}
