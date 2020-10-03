use serde::{Serialize, de::DeserializeOwned};
use crate::parser::types::Field;
use crate::{Type, InputValueType, OutputValueType, ContextSelectionSet, Positioned, ServerResult, ServerError};
use crate::registry::{Registry, MetaType};
use std::borrow::Cow;
use async_trait::async_trait;

/// A GraphQL scalar.
///
/// You can implement the trait to create a custom scalar.
///
/// # Examples
///
/// ```rust
/// use async_graphql::*;
/// use serde::*;
///
/// #[derive(Scalar, Serialize, Deserialize)]
/// #[serde(transparent)]
/// struct MyInt(i32);
/// ```
pub trait ScalarType: Serialize + DeserializeOwned + Send {}

macro_rules! external_scalar {
    ($(
        $(#[doc = $doc:literal])*
        $(#[cfg($($cfg:tt)*)])?
        [$($generics:tt)*] $name:ty = $gql_typename:literal,
    )*) => {
        $(
            $(#[cfg($($cfg)*)])?
            impl<$($generics)*> ScalarType for $name {}
            $(#[cfg($($cfg)*)])?
            impl<$($generics)*> Type for $name {
                fn type_name() -> Cow<'static, str> {
                    Cow::Borrowed($gql_typename)
                }

                fn create_type_info(registry: &mut Registry) -> String {
                    registry.create_type::<Self, _>(|_| MetaType::Scalar {
                        name: $gql_typename.to_string(),
                        description: Some(concat!($($doc),*)),
                    })
                }
            }
            $(#[cfg($($cfg)*)])?
            impl<$($generics)*> InputValueType for $name {}

            #[async_trait]
            $(#[cfg($($cfg)*)])?
            impl<$($generics)*> OutputValueType for $name {
                async fn resolve(
                    &self,
                    ctx: &ContextSelectionSet<'_>,
                    _field: &Positioned<Field>
                ) -> ServerResult<serde_json::Value> {
                    serde_json::to_value(self)
                        .map_err(|e| ServerError::new(e.to_string()).at(ctx.item.pos))
                }
            }
        )*
    };
}

external_scalar! {
    /// The `Boolean` scalar type represents `true` or `false`.
    [] bool = "Boolean",

    /// The `Char` scalar type represents a unicode char.
    /// The input and output values are a string, and there can only be one unicode character in
    /// this string.
    [] char = "Char",

    /// The `Float` scalar type represents signed double-precision fractional values as specified by
    /// [IEEE 754](https://en.wikipedia.org/wiki/IEEE_floating_point).
    [] f64 = "Float",
    /// The `Float` scalar type represents signed double-precision fractional values as specified by
    /// [IEEE 754](https://en.wikipedia.org/wiki/IEEE_floating_point).
    [] f32 = "Float",

    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] i8 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] i16 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] i32 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] u8 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] u16 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] std::num::NonZeroI8 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] std::num::NonZeroI16 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] std::num::NonZeroI32 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] std::num::NonZeroU8 = "Int",
    /// The `Int` scalar type represents non-fractional whole numeric values.
    [] std::num::NonZeroU16 = "Int",

    /// A signed 64-bit integer.
    [] i64 = "I64",
    /// A signed 128-bit integer.
    [] i128 = "I128",
    /// An unsigned 32-bit integer.
    [] u32 = "U32",
    /// An unsigned 64-bit integer.
    [] u64 = "U64",
    /// An unsigned 128-bit integer.
    [] u128 = "U128",
    /// A signed integer equivalent to the word size of the GraphQL server.
    [] isize = "Isize",
    /// An unsigned integer equivalent to the word size of the GraphQL server.
    [] usize = "Usize",
    /// A signed 64-bit integer.
    [] std::num::NonZeroI64 = "I64",
    /// A signed 128-bit integer.
    [] std::num::NonZeroI128 = "I128",
    /// An unsigned 32-bit integer.
    [] std::num::NonZeroU32 = "U32",
    /// An unsigned 64-bit integer.
    [] std::num::NonZeroU64 = "U64",
    /// An unsigned 128-bit integer.
    [] std::num::NonZeroU128 = "U128",
    /// A signed integer equivalent to the word size of the GraphQL server.
    [] std::num::NonZeroIsize = "Isize",
    /// An unsigned integer equivalent to the word size of the GraphQL server.
    [] std::num::NonZeroUsize = "Usize",

    /// The `String` scalar type represents textual data, represented as UTF-8 character sequences.
    /// The String type is most often used by GraphQL to represent free-form human-readable text.
    [] String = "String",

    /// Any map of keys to values.
    [
        K: Serialize + DeserializeOwned + Ord + Send,
        V: Serialize + DeserializeOwned + Send
    ] std::collections::BTreeMap<K, V> = "Map",
    /// Any map of keys to values.
    [
        K: Serialize + DeserializeOwned + Eq + std::hash::Hash + Send,
        V: Serialize + DeserializeOwned + Send,
        H: std::hash::BuildHasher + Default + Send,
    ] std::collections::HashMap<K, V, H> = "Map",

    /// A 12-byte BSON object identifier.
    #[cfg(feature = "bson")]
    [] bson::oid::ObjectId = "ObjectId",

    /// A UTC date and time.
    ///
    /// The input/output is a string in RFC3339 format.
    #[cfg(all(feature = "bson", feature = "chrono"))]
    [] bson::DateTime = "DateTime",

    /// A date and time with a fixed offset.
    ///
    /// The input/output is a string in RFC3339 format.
    #[cfg(feature = "chrono")]
    [] chrono::DateTime<chrono::FixedOffset> = "DateTime",

    /// A date and time with the local offset.
    ///
    /// The input/output is a string in RFC3339 format.
    #[cfg(feature = "chrono")]
    [] chrono::DateTime<chrono::Local> = "DateTime",

    /// A date and time in UTC.
    ///
    /// The input/output is a string in RFC3339 format.
    #[cfg(feature = "chrono")]
    [] chrono::DateTime<chrono::Utc> = "DateTime",

    /// A date.
    ///
    /// The input/output is a string in RFC3339 format.
    #[cfg(feature = "chrono")]
    [] chrono::NaiveDate = "Date",

    /// A time.
    #[cfg(feature = "chrono")]
    [] chrono::NaiveTime = "Time",

    /// A date and time.
    #[cfg(feature = "chrono")]
    [] chrono::NaiveDateTime = "DateTime",

    /// A time zone.
    #[cfg(feature = "chrono_tz")]
    [] chrono_tz::Tz = "TimeZone",

    /// A URL.
    #[cfg(feature = "url")]
    [] url::Url = "Url",

    /// A UUID.
    #[cfg(feature = "uuid")]
    [] uuid::Uuid = "UUID",
}
