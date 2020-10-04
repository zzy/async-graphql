use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Bound, RangeBounds};

use serde::de::{self, Deserialize, Deserializer, Visitor, SeqAccess};

use super::InputValueValidator;

/// A validator that asserts the value is in the range.
///
/// This validator works for any range that implements `RangeBounds`, and any type that implements
/// `Deserialize + PartialOrd + Display` (`Display` is used to make error messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InRange<R, Idx> {
    /// The range which constrains the input value.
    pub range: R,
    marker: PhantomData<Idx>,
}

impl<R, Idx> InRange<R, Idx> {
    /// Create a new range validator.
    pub fn new(range: R) -> Self {
        Self {
            range,
            marker: PhantomData,
        }
    }
}

impl<'de, R, Idx> InputValueValidator<'de> for InRange<R, Idx>
where
    R: RangeBounds<Idx> + Send + Sync,
    Idx: Deserialize<'de> + PartialOrd + Display + Send + Sync,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        range_contains(
            &self.0,
            Idx::deserialize(deserializer)?,
            "value",
            None,
            ("low", "high"),
        )
            .map_err(D::Error::custom)
    }
}

/// A validator that asserts a list's length to be in a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListLength<R> {
    /// The range the list's length must be inside.
    pub range: R,
}

impl<R> ListLength<R> {
    /// Create a new list length validator.
    pub fn new(range: R) -> Self {
        Self {
            range,
        }
    }
}

impl<'de, R> InputValueValidator<'de> for ListLength<R>
where
    R: RangeBounds<usize> + Send + Sync,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        struct LengthCounter;
        impl<'de> Visitor<'de> for LengthCounter {
            type Value = usize;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("a list")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut count = 0;
                while let Some(_) = seq.next_element()? {
                    count += 1;
                }
                Ok(count)
            }
        }

        range_contains(
            &self.range,
            deserializer.deserialize_seq(LengthCounter)?,
            "list",
            Some("items"),
            ("short", "long"),
        )
            .map_err(D::Error::custom)
    }
}

/// A validator that asserts a string's length in bytes to be in a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringLength<R> {
    /// The range the string's length must be inside.
    pub range: R,
}

impl<R> StringLength<R> {
    /// Create a new string length validator.
    pub fn new(range: R) -> Self {
        Self {
            range,
        }
    }
}

impl<'de, R> InputValueValidator<'de> for StringLength<R>
where
    R: RangeBounds<usize> + Send + Sync,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        struct StrLen;
        impl<'de> Visitor<'de> for StrLen {
            type Value = usize;
            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(v.len())
            }
        }

        range_contains(
            &self.range,
            deserializer.deserialize_str(StrLen)?,
            "string",
            Some("bytes"),
            ("short", "long"),
        )
            .map_err(D::Error::custom)
    }
}

/// A validator that asserts the number of characters in a string to be in a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringChars<R> {
    /// The range that contains the number of characters in the string.
    pub range: R,
}

impl<R> StringChars<R> {
    /// Create a new character validator.
    pub fn new(range: R) -> Self {
        Self { range, }
    }
}

impl<'de, R> InputValueValidator<'de> for StringChars<R>
where
    R: RangeBounds<usize> + Send + Sync,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        struct Chars;
        impl<'de> Visitor<'de> for Chars {
            type Value = usize;
            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(v.chars().count())
            }
        }

        range_contains(
            &self.range,
            deserializer.deserialize_str(Chars)?,
            "string",
            Some("characters"),
            ("short", "long"),
        )
            .map_err(D::Error::custom)
    }
}

fn range_contains<'a, Idx: PartialOrd + Display>(
    range: &'a impl RangeBounds<Idx>,
    value: &Idx,
    item: &'a str,
    items: Option<&'a str>,
    error_types: (&'a str, &'a str),
) -> Result<(), fmt::Arguments<'a>> {
    let items = items.map_or(format_args!(""), |items| format_args!(" {} long", items));

    match (range.start_bound(), range.end_bound()) {
        (Bound::Included(bound), _) if value < bound => Err(
            format_args!("{} is too {}, must be {} or above{}", item, error_types.0, bound, items)
        ),
        (Bound::Excluded(bound), _) if value <= bound => Err(
            format_args!("{} is too {}, must be above {}{}", item, error_types.0, bound, items)
        ),
        (_, Bound::Included(bound)) if value > bound => Err(
            format_args!("{} is too {}, must be {} or below{}", item, error_types.1, bound, items)
        ),
        (_, Bound::Excluded(bound)) if value >= bound => Err(
            format_args!("{} is too {}, must be below {}{}", item, error_types.1, bound, items)
        ),
        _ => Ok(()),
    }
}
