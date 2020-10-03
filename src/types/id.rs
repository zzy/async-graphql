use crate::Scalar;
#[cfg(feature = "bson")]
use bson::oid::{self, ObjectId};
use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer, Visitor};
use std::fmt::{self, Formatter};
use std::convert::TryFrom;
use std::num::ParseIntError;
use std::ops::{Deref, DerefMut};

/// ID scalar.
///
/// It deserializes from strings and integers, and the output is a string.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Serialize, Scalar)]
#[graphql(internal)]
#[serde(transparent)]
pub struct ID(pub String);

impl Deref for ID {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ID {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: std::fmt::Display> From<T> for ID {
    fn from(value: T) -> Self {
        ID(value.to_string())
    }
}

impl From<ID> for String {
    fn from(id: ID) -> Self {
        id.0
    }
}

macro_rules! try_from_integers {
    ($($ty:ty),*) => {
        $(
           impl TryFrom<ID> for $ty {
                type Error = ParseIntError;

                fn try_from(id: ID) -> std::result::Result<Self, Self::Error> {
                    id.0.parse()
                }
            }
         )*
    };
}

try_from_integers!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, isize, usize);

#[cfg(feature = "uuid")]
impl TryFrom<ID> for uuid::Uuid {
    type Error = uuid::Error;

    fn try_from(id: ID) -> std::result::Result<Self, Self::Error> {
        uuid::Uuid::parse_str(&id.0)
    }
}

#[cfg(feature = "bson")]
impl TryFrom<ID> for ObjectId {
    type Error = oid::Error;

    fn try_from(id: ID) -> std::result::Result<Self, oid::Error> {
        ObjectId::with_string(&id.0)
    }
}

impl PartialEq<&str> for ID {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

impl<'de> Deserialize<'de> for ID {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IDVisitor;
        impl<'de> Visitor<'de> for IDVisitor {
            type Value = ID;
            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("a GraphQL ID")
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(ID(v.to_string()))
            }
            fn visit_i128<E: de::Error>(self, v: i128) -> Result<Self::Value, E> {
                Ok(ID(v.to_string()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ID(v.to_string()))
            }
            fn visit_u128<E: de::Error>(self, v: u128) -> Result<Self::Value, E> {
                Ok(ID(v.to_string()))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(ID(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(ID(v))
            }
        }

        deserializer.deserialize_string(IDVisitor)
    }
}
