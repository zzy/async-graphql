//! Input value validators

use serde::Deserializer;

mod range;
mod string;

pub use range::*;
pub use string::*;

/// Input value validator
///
/// You can create your own input value validator by implementing this trait.
///
/// # Examples
///
/// ```no_run
/// use async_graphql::*;
/// use async_graphql::validators::{Email, MAC, InRange};
///
/// struct QueryRoot;
///
/// #[Object]
/// impl QueryRoot {
///     // Input is email address
///     async fn value1(&self, #[graphql(validator(Email))] email: String) -> i32 {
///         unimplemented!()
///     }
///
///     // Input is email or MAC address (requiring a colon)
///     async fn value2(&self, #[graphql(validator(or(Email, MAC(colon = "true"))))] email_or_mac: String) -> i32 {
///         unimplemented!()
///     }
///
///     // Input is integer between 100 and 200
///     async fn value3(&self, #[graphql(validator(InRange = "100..=200_i32"))] value: i32) -> i32 {
///         unimplemented!()
///     }
/// }
/// ```
///
/// # Validator syntax
///
/// Inside validator attributes, `name` is equivalent to `name()`, and `name = value` is equivalent
/// to `name(value)` - you can use whichever you prefer. So `MAC(colon = "true")` is equivalent to
/// `MAC(colon("true"))`, `Email` is equivalent to `Email()` and `InRange = "100..=200_i32"` is
/// equivalent to `InRange("100..=200_i32")`.
///
/// Each validator takes a list of expressions to be passed to the constructor (if none are provided 
/// the type's `Default` implementation is used, otherwise it uses the method named `new`). It then
/// takes additional methods to call on the type. For example `MAC(colon = "true")` is equivalent to
/// the expression `MAC::default().colon(true)`, and `InRange = "100..=200_i32"` is equivalent to
/// the expression `InRange::new(100..=200_i32)`.
pub trait InputValueValidator<'de>: Send + Sync {
    /// Check whether the value held by the deserializer is valid.
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error>;
}

/// An extension trait for `InputValueValidator`
pub trait InputValueValidatorExt<'de>: InputValueValidator<'de> + Sized {
    /// Merge the two validators and succeed only if both validators are successful.
    fn and<R: InputValueValidator<'de>>(self, other: R) -> And<Self, R> {
        And(self, other)
    }

    /// Merge two validators, and succeed when either validator verifies successfully.
    fn or<R: InputValueValidator<'de>>(self, other: R) -> Or<Self, R> {
        Or(self, other)
    }
}

impl<'de, I: InputValueValidator<'de>> InputValueValidatorExt<'de> for I {}

/// Validator for `InputValueValidatorExt::and`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct And<A, B>(pub A, pub B);

impl<'de, A, B> InputValueValidator<'de> for And<A, B>
where
    A: InputValueValidator<'de>,
    B: InputValueValidator<'de>,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        self.0.validate(deserializer.clone())
            .and_then(|()| self.1.validate(deserializer))
    }
}

/// Validator for `InputValueValidator::or`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Or<A, B>(pub A, pub B);

impl<'de, A, B> InputValueValidator<'de> for Or<A, B>
where
    A: InputValueValidator<'de>,
    B: InputValueValidator<'de>,
{
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        self.0.validate(deserializer.clone())
            .or_else(|_| self.1.validate(deserializer))
    }
}
