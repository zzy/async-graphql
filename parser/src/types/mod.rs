//! GraphQL types.
//!
//! The two root types are [`ExecutableDocument`](struct.ExecutableDocument.html) and
//! [`ServiceDocument`](struct.ServiceDocument.html), representing an executable GraphQL query and a
//! GraphQL service respectively.
//!
//! This follows the [June 2018 edition of the GraphQL spec](https://spec.graphql.org/June2018/).

use crate::pos::Positioned;
use serde::de::value::{MapDeserializer, SeqDeserializer, StringDeserializer, BorrowedStrDeserializer};
use serde::de::{self, Deserializer, Error as _, IntoDeserializer, Unexpected, Visitor};
use serde::ser::{Error as _, Serializer};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{hash_map, BTreeMap, HashMap};
use std::convert::{TryFrom, TryInto};
use std::fmt::{self, Display, Formatter, Write};
use std::marker::PhantomData;
use std::ops::Deref;

pub use executable::*;
pub use serde_json::Number;
pub use service::*;

mod executable;
mod service;

/// The type of an operation; `query`, `mutation` or `subscription`.
///
/// [Reference](https://spec.graphql.org/June2018/#OperationType).
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OperationType {
    /// A query.
    Query,
    /// A mutation.
    Mutation,
    /// A subscription.
    Subscription,
}

impl Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
            Self::Subscription => "subscription",
        })
    }
}

/// A GraphQL type, for example `String` or `[String!]!`.
///
/// [Reference](https://spec.graphql.org/June2018/#Type).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Type {
    /// The base type.
    pub base: BaseType,
    /// Whether the type is nullable.
    pub nullable: bool,
}

impl Type {
    /// Create a type from the type string.
    #[must_use]
    pub fn new(ty: &str) -> Option<Self> {
        let (nullable, ty) = if let Some(rest) = ty.strip_suffix('!') {
            (false, rest)
        } else {
            (true, ty)
        };

        Some(Self {
            base: if let Some(ty) = ty.strip_prefix('[') {
                BaseType::List(Box::new(Self::new(ty.strip_suffix(']')?)?))
            } else {
                BaseType::Named(Name::new(ty.to_owned()).ok()?)
            },
            nullable,
        })
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.base.fmt(f)?;
        if !self.nullable {
            f.write_char('!')?;
        }
        Ok(())
    }
}

/// A GraphQL base type, for example `String` or `[String!]`. This does not include whether the
/// type is nullable; for that see [Type](struct.Type.html).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BaseType {
    /// A named type, such as `String`.
    Named(Name),
    /// A list type, such as `[String]`.
    List(Box<Type>),
}

impl Display for BaseType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => f.write_str(name),
            Self::List(ty) => write!(f, "[{}]", ty),
        }
    }
}

/// A resolved GraphQL value, for example `1` or `"Hello World!"`.
///
/// It can be serialized and deserialized. Enums will be converted to strings. `Enum` cannot be
/// deserialized.
///
/// [Reference](https://spec.graphql.org/June2018/#Value).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConstValue {
    /// `null`.
    Null,
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// A boolean.
    Boolean(bool),
    /// An enum. These are typically in `SCREAMING_SNAKE_CASE`.
    #[serde(skip_deserializing)]
    Enum(Name),
    /// A list of values.
    List(Vec<ConstValue>),
    /// An object. This is a map of keys to values.
    Object(BTreeMap<Name, ConstValue>),
}

impl ConstValue {
    /// Convert this `ConstValue` into a `Value`.
    #[must_use]
    pub fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Number(num) => Value::Number(num),
            Self::String(s) => Value::String(s),
            Self::Boolean(b) => Value::Boolean(b),
            Self::Enum(v) => Value::Enum(v),
            Self::List(items) => {
                Value::List(items.into_iter().map(ConstValue::into_value).collect())
            }
            Self::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(key, value)| (key, value.into_value()))
                    .collect(),
            ),
        }
    }

    /// Attempt to convert the value into JSON. This is equivalent to the `TryFrom` implementation.
    ///
    /// # Errors
    ///
    /// Fails if serialization fails (see enum docs for more info).
    pub fn into_json(self) -> serde_json::Result<serde_json::Value> {
        self.try_into()
    }

    /// Attempt to convert JSON into a value. This is equivalent to the `TryFrom` implementation.
    ///
    /// # Errors
    ///
    /// Fails if deserialization fails (see enum docs for more info).
    pub fn from_json(json: serde_json::Value) -> serde_json::Result<Self> {
        json.try_into()
    }
}

impl Default for ConstValue {
    fn default() -> Self {
        Self::Null
    }
}

impl Display for ConstValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(num) => write!(f, "{}", *num),
            Self::String(val) => write_quoted(val, f),
            Self::Boolean(true) => f.write_str("true"),
            Self::Boolean(false) => f.write_str("false"),
            Self::Null => f.write_str("null"),
            Self::Enum(name) => f.write_str(name),
            Self::List(items) => write_list(items, f),
            Self::Object(map) => write_object(map, f),
        }
    }
}

impl TryFrom<serde_json::Value> for ConstValue {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        Self::deserialize(value)
    }
}
impl TryFrom<ConstValue> for serde_json::Value {
    type Error = serde_json::Error;
    fn try_from(value: ConstValue) -> Result<Self, Self::Error> {
        serde_json::to_value(value)
    }
}

impl<'de> Deserializer<'de> for ConstValue {
    type Error = de::value::Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            Self::Null => visitor.visit_unit(),
            Self::Number(n) => n.deserialize_any(visitor).map_err(de::Error::custom),
            Self::String(s) => visitor.visit_string(s),
            Self::Boolean(b) => visitor.visit_bool(b),
            Self::Enum(v) => visitor.visit_enum(v.into_deserializer()),
            Self::List(a) => a.into_deserializer().deserialize_any(visitor),
            Self::Object(o) => o.into_deserializer().deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            Self::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
impl<'de> IntoDeserializer<'de> for ConstValue {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> Deserializer<'de> for &'de ConstValue {
    type Error = de::value::Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            ConstValue::Null => visitor.visit_unit(),
            ConstValue::Number(n) => n.deserialize_any(visitor).map_err(de::Error::custom),
            ConstValue::String(s) => visitor.visit_borrowed_str(&s),
            &ConstValue::Boolean(b) => visitor.visit_bool(b),
            ConstValue::Enum(v) => visitor.visit_enum(v.into_deserializer()),
            ConstValue::List(a) => SeqDeserializer::new(a.iter()).deserialize_any(visitor),
            ConstValue::Object(o) => MapDeserializer::new(o.iter()).deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            ConstValue::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
impl<'de> IntoDeserializer<'de> for &'de ConstValue {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

/// A GraphQL value, for example `1`, `$name` or `"Hello World!"`. This is
/// [`ConstValue`](enum.ConstValue.html) with variables.
///
/// It can be serialized and deserialized. Enums will be converted to strings. Attempting to
/// serialize `Variable` will fail, and `Enum` and `Variable` cannot be deserialized.
///
/// [Reference](https://spec.graphql.org/June2018/#Value).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Value {
    /// A variable, without the `$`.
    #[serde(serialize_with = "fail_serialize_variable", skip_deserializing)]
    Variable(Name),
    /// `null`.
    Null,
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// A boolean.
    Boolean(bool),
    /// An enum. These are typically in `SCREAMING_SNAKE_CASE`.
    #[serde(skip_deserializing)]
    Enum(Name),
    /// A list of values.
    List(Vec<Value>),
    /// An object. This is a map of keys to values.
    Object(BTreeMap<Name, Value>),
}

impl Value {
    /// Deserialize the value using a function to get the variables.
    pub fn deserializer<F, E>(self, variables: &F) -> ValueDeserializer<'_, F, E> {
        ValueDeserializer::new(self, variables)
    }

    /// Attempt to convert the value into JSON. This is equivalent to the `TryFrom` implementation.
    ///
    /// # Errors
    ///
    /// Fails if serialization fails (see enum docs for more info).
    pub fn into_json(self) -> serde_json::Result<serde_json::Value> {
        self.try_into()
    }

    /// Attempt to convert JSON into a value. This is equivalent to the `TryFrom` implementation.
    ///
    /// # Errors
    ///
    /// Fails if deserialization fails (see enum docs for more info).
    pub fn from_json(json: serde_json::Value) -> serde_json::Result<Self> {
        json.try_into()
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variable(name) => write!(f, "${}", name),
            Self::Number(num) => write!(f, "{}", *num),
            Self::String(val) => write_quoted(val, f),
            Self::Boolean(true) => f.write_str("true"),
            Self::Boolean(false) => f.write_str("false"),
            Self::Null => f.write_str("null"),
            Self::Enum(name) => f.write_str(name),
            Self::List(items) => write_list(items, f),
            Self::Object(map) => write_object(map, f),
        }
    }
}

impl From<ConstValue> for Value {
    fn from(value: ConstValue) -> Self {
        value.into_value()
    }
}

impl TryFrom<serde_json::Value> for Value {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        Self::deserialize(value)
    }
}
impl TryFrom<Value> for serde_json::Value {
    type Error = serde_json::Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        serde_json::to_value(value)
    }
}

fn fail_serialize_variable<S: Serializer>(_: &str, _: S) -> Result<S::Ok, S::Error> {
    Err(S::Error::custom("cannot serialize variable"))
}

fn write_quoted(s: &str, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_char('"')?;
    for c in s.chars() {
        match c {
            '\r' => f.write_str("\\r"),
            '\n' => f.write_str("\\n"),
            '\t' => f.write_str("\\t"),
            '"' => f.write_str("\\\""),
            '\\' => f.write_str("\\\\"),
            c if c.is_control() => write!(f, "\\u{:04}", c as u32),
            c => f.write_char(c),
        }?
    }
    f.write_char('"')
}
fn write_list<T: Display>(list: impl IntoIterator<Item = T>, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_char('[')?;
    for item in list {
        item.fmt(f)?;
        f.write_char(',')?;
    }
    f.write_char(']')
}
fn write_object<K: Display, V: Display>(
    object: impl IntoIterator<Item = (K, V)>,
    f: &mut Formatter<'_>,
) -> fmt::Result {
    f.write_char('{')?;
    for (name, value) in object {
        write!(f, "{}: {},", name, value)?;
    }
    f.write_char('}')
}

/// A deserializer of `Value`s.
#[derive(Debug, Clone)]
pub struct ValueDeserializer<'a, F, E> {
    /// The value being deserialized.
    pub value: Value,
    /// The function used to access the variables that are used in deserialization.
    pub variables: &'a F,
    marker: PhantomData<E>,
}

impl<'a, F, E> ValueDeserializer<'a, F, E> {
    /// Construct a new `ValueDeserializer`.
    #[must_use]
    pub fn new(value: Value, variables: &'a F) -> Self {
        Self {
            value,
            variables,
            marker: PhantomData,
        }
    }
}

fn get_variable<'de, F, E, T>(variables: F, name: &Name) -> Result<T::Deserializer, E>
where
    F: FnOnce(&Name) -> Option<T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    (variables)(name)
        .ok_or_else(|| E::custom(format_args!("variable {} is not defined", name)))
        .map(T::into_deserializer)
}

impl<'a, 'de, F, E, T> Deserializer<'de> for ValueDeserializer<'a, F, E>
where
    F: Fn(&Name) -> Option<T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let variables = self.variables;
        match self.value {
            Value::Variable(name) => get_variable(variables, &name)?.deserialize_any(visitor),
            Value::Null => visitor.visit_unit(),
            Value::Number(n) => n.deserialize_any(visitor).map_err(E::custom),
            Value::String(s) => visitor.visit_string(s),
            Value::Boolean(b) => visitor.visit_bool(b),
            Value::Enum(v) => visitor.visit_enum(v.into_deserializer()),
            Value::List(a) => SeqDeserializer::new(
                a.into_iter()
                    .map(|v| ValueDeserializer::new(v, variables)),
            )
            .deserialize_any(visitor),
            Value::Object(o) => MapDeserializer::new(
                o.into_iter()
                    .map(|(k, v)| (k, ValueDeserializer::new(v, variables))),
            )
            .deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let variables = self.variables;
        match self.value {
            Value::Variable(name) => get_variable(variables, &name)?.deserialize_option(visitor),
            Value::Null => visitor.visit_none(),
            value => visitor.visit_some(ValueDeserializer::new(value, variables)),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'a, 'de, F, E, T> IntoDeserializer<'de, E> for ValueDeserializer<'a, F, E>
where
    F: Fn(&Name) -> Option<T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Deserializer = Self;
    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

/// A const GraphQL directive, such as `@deprecated(reason: "Use the other field)`. This differs
/// from [`Directive`](struct.Directive.html) in that it uses [`ConstValue`](enum.ConstValue.html)
/// instead of [`Value`](enum.Value.html).
///
/// [Reference](https://spec.graphql.org/June2018/#Directive).
#[derive(Debug, Clone)]
pub struct ConstDirective {
    /// The name of the directive.
    pub name: Positioned<Name>,
    /// The arguments to the directive.
    pub arguments: Vec<(Positioned<Name>, Positioned<ConstValue>)>,
}

impl ConstDirective {
    /// Convert this `ConstDirective` into a `Directive`.
    #[must_use]
    pub fn into_directive(self) -> Directive {
        Directive {
            name: self.name,
            arguments: self
                .arguments
                .into_iter()
                .map(|(name, value)| (name, value.map(ConstValue::into_value)))
                .collect(),
        }
    }

    /// Get the argument with the given name.
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<ConstValue>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}

/// A GraphQL directive, such as `@deprecated(reason: "Use the other field")`.
///
/// [Reference](https://spec.graphql.org/June2018/#Directive).
#[derive(Debug, Clone)]
pub struct Directive {
    /// The name of the directive.
    pub name: Positioned<Name>,
    /// The arguments to the directive.
    pub arguments: Vec<(Positioned<Name>, Positioned<Value>)>,
}

impl Directive {
    /// Get the argument with the given name.
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&Positioned<Value>> {
        self.arguments
            .iter()
            .find(|item| item.0.node == name)
            .map(|item| &item.1)
    }
}

/// A GraphQL name. This is a newtype wrapper around a string with the addition guarantee that it
/// is a valid GraphQL name (follows the regex `[_A-Za-z][_0-9A-Za-z]*`).
///
/// [Reference](https://spec.graphql.org/June2018/#Name).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Name(String);

impl Name {
    /// Check whether the name is valid (follows the regex `[_A-Za-z][_0-9A-Za-z]*`).
    #[must_use]
    pub fn is_valid(name: &str) -> bool {
        let mut bytes = name.bytes();
        bytes
            .next()
            .map_or(false, |c| c.is_ascii_alphabetic() || c == b'_')
            && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
    }

    /// Create a new name without checking whether it is valid or not. This will always check in
    /// debug mode.
    ///
    /// This function is not `unsafe` because an invalid name does not cause UB, but care should be
    /// taken to make sure it is a valid name.
    #[must_use]
    pub fn new_unchecked(name: String) -> Self {
        debug_assert!(Self::is_valid(&name));
        Self(name)
    }

    /// Create a new name, checking whether it is valid. Returns ownership of the string if it
    /// fails.
    ///
    /// # Errors
    ///
    /// Fails if the name is not a valid name.
    pub fn new(name: String) -> Result<Self, String> {
        if Self::is_valid(&name) {
            Ok(Self(name))
        } else {
            Err(name)
        }
    }

    /// Get the name as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert the name to a `String`.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<Name> for String {
    fn from(name: Name) -> Self {
        name.0
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl PartialEq<String> for Name {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}
impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}
impl PartialEq<Name> for String {
    fn eq(&self, other: &Name) -> bool {
        other == self
    }
}
impl PartialEq<Name> for str {
    fn eq(&self, other: &Name) -> bool {
        other == self
    }
}
impl<'a> PartialEq<&'a str> for Name {
    fn eq(&self, other: &&'a str) -> bool {
        self == *other
    }
}
impl<'a> PartialEq<Name> for &'a str {
    fn eq(&self, other: &Name) -> bool {
        other == self
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?)
            .map_err(|s| D::Error::invalid_value(Unexpected::Str(&s), &"a GraphQL name"))
    }
}

impl<'de, E: de::Error> IntoDeserializer<'de, E> for Name {
    type Deserializer = StringDeserializer<E>;
    fn into_deserializer(self) -> Self::Deserializer {
        self.into_string().into_deserializer()
    }
}
impl<'de, E: de::Error> IntoDeserializer<'de, E> for &'de Name {
    type Deserializer = BorrowedStrDeserializer<'de, E>;
    fn into_deserializer(self) -> Self::Deserializer {
        BorrowedStrDeserializer::new(self.as_str())
    }
}

#[cfg(test)]
#[test]
fn test_valid_names() {
    assert!(Name::is_valid("valid_name"));
    assert!(Name::is_valid("numbers123_456_789abc"));
    assert!(Name::is_valid("MiXeD_CaSe"));
    assert!(Name::is_valid("_"));
    assert!(!Name::is_valid("invalid name"));
    assert!(!Name::is_valid("123and_text"));
}
