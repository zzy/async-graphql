//! Query context.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::fmt::{self, Debug, Display, Formatter};
use std::ops::Deref;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use async_graphql_value::Value as InputValue;
use fnv::FnvHashMap;
use http::header::{AsHeaderName, HeaderMap, IntoHeaderName};
use serde::de::{Deserialize, Deserializer};
use serde::ser::{SerializeSeq, Serializer};
use serde::Serialize;

use crate::extensions::Extensions;
use crate::parser::types::{
    Directive, Field, FragmentDefinition, OperationDefinition, Selection, SelectionSet,
};
use crate::schema::SchemaEnv;
use crate::{
    Error, InputType, Lookahead, Name, Pos, Positioned, Result, ServerError, ServerResult,
    UploadValue, Value,
};

/// Variables of a query.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(transparent)]
pub struct Variables(pub BTreeMap<Name, Value>);

impl Display for Variables {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, (name, value)) in self.0.iter().enumerate() {
            write!(f, "{}{}: {}", if i == 0 { "" } else { ", " }, name, value)?;
        }
        f.write_str("}")
    }
}

impl<'de> Deserialize<'de> for Variables {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(
            <Option<BTreeMap<Name, Value>>>::deserialize(deserializer)?.unwrap_or_default(),
        ))
    }
}

impl Variables {
    /// Get the variables from a GraphQL value.
    ///
    /// If the value is not a map, then no variables will be returned.
    #[must_use]
    pub fn from_value(value: Value) -> Self {
        match value {
            Value::Object(obj) => Self(obj),
            _ => Self::default(),
        }
    }

    /// Get the values from a JSON value.
    ///
    /// If the value is not a map or the keys of a map are not valid GraphQL names, then no
    /// variables will be returned.
    #[must_use]
    pub fn from_json(value: serde_json::Value) -> Self {
        Value::from_json(value)
            .map(Self::from_value)
            .unwrap_or_default()
    }

    /// Get the variables as a GraphQL value.
    #[must_use]
    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }

    pub(crate) fn variable_path(&mut self, path: &str) -> Option<&mut Value> {
        let mut parts = path.strip_prefix("variables.")?.split('.');

        let initial = self.0.get_mut(parts.next().unwrap())?;

        parts.try_fold(initial, |current, part| match current {
            Value::List(list) => part
                .parse::<u32>()
                .ok()
                .and_then(|idx| usize::try_from(idx).ok())
                .and_then(move |idx| list.get_mut(idx)),
            Value::Object(obj) => obj.get_mut(part),
            _ => None,
        })
    }
}

impl From<Variables> for Value {
    fn from(variables: Variables) -> Self {
        variables.into_value()
    }
}

/// Schema/Context data.
///
/// This is a type map, allowing you to store anything inside it.
#[derive(Default)]
pub struct Data(FnvHashMap<TypeId, Box<dyn Any + Sync + Send>>);

impl Deref for Data {
    type Target = FnvHashMap<TypeId, Box<dyn Any + Sync + Send>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Data {
    /// Insert data.
    pub fn insert<D: Any + Send + Sync>(&mut self, data: D) {
        self.0.insert(TypeId::of::<D>(), Box::new(data));
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Data").finish()
    }
}

/// Context for `SelectionSet`
pub type ContextSelectionSet<'a> = ContextBase<'a, &'a Positioned<SelectionSet>>;

/// Context object for resolve field
pub type Context<'a> = ContextBase<'a, &'a Positioned<Field>>;

/// A segment in the path to the current query.
///
/// This is a borrowed form of [`PathSegment`](enum.PathSegment.html) used during execution instead
/// of passed back when errors occur.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
pub enum QueryPathSegment<'a> {
    /// We are currently resolving an element in a list.
    Index(usize),
    /// We are currently resolving a field in an object.
    Name(&'a str),
}

/// A path to the current query.
///
/// The path is stored as a kind of reverse linked list.
#[derive(Debug, Clone, Copy)]
pub struct QueryPathNode<'a> {
    /// The parent node to this, if there is one.
    pub parent: Option<&'a QueryPathNode<'a>>,

    /// The current path segment being resolved.
    pub segment: QueryPathSegment<'a>,
}

impl<'a> serde::Serialize for QueryPathNode<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        self.try_for_each(|segment| seq.serialize_element(segment))?;
        seq.end()
    }
}

impl<'a> Display for QueryPathNode<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;
        self.try_for_each(|segment| {
            if !first {
                write!(f, ".")?;
            }
            first = false;

            match segment {
                QueryPathSegment::Index(idx) => write!(f, "{}", *idx),
                QueryPathSegment::Name(name) => write!(f, "{}", name),
            }
        })
    }
}

impl<'a> QueryPathNode<'a> {
    /// Get the current field name.
    ///
    /// This traverses all the parents of the node until it finds one that is a field name.
    pub fn field_name(&self) -> &str {
        std::iter::once(self)
            .chain(self.parents())
            .find_map(|node| match node.segment {
                QueryPathSegment::Name(name) => Some(name),
                QueryPathSegment::Index(_) => None,
            })
            .unwrap()
    }

    /// Get the path represented by `Vec<String>`; numbers will be stringified.
    #[must_use]
    pub fn to_string_vec(&self) -> Vec<String> {
        let mut res = Vec::new();
        self.for_each(|s| {
            res.push(match s {
                QueryPathSegment::Name(name) => (*name).to_string(),
                QueryPathSegment::Index(idx) => idx.to_string(),
            });
        });
        res
    }

    /// Iterate over the parents of the node.
    pub fn parents(&self) -> Parents<'_> {
        Parents(self)
    }

    pub(crate) fn for_each<F: FnMut(&QueryPathSegment<'a>)>(&self, mut f: F) {
        let _ = self.try_for_each::<std::convert::Infallible, _>(|segment| {
            f(segment);
            Ok(())
        });
    }

    pub(crate) fn try_for_each<E, F: FnMut(&QueryPathSegment<'a>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        self.try_for_each_ref(&mut f)
    }

    fn try_for_each_ref<E, F: FnMut(&QueryPathSegment<'a>) -> Result<(), E>>(
        &self,
        f: &mut F,
    ) -> Result<(), E> {
        if let Some(parent) = &self.parent {
            parent.try_for_each_ref(f)?;
        }
        f(&self.segment)
    }
}

/// An iterator over the parents of a [`QueryPathNode`](struct.QueryPathNode.html).
#[derive(Debug, Clone)]
pub struct Parents<'a>(&'a QueryPathNode<'a>);

impl<'a> Parents<'a> {
    /// Get the current query path node, which the next call to `next` will get the parents of.
    #[must_use]
    pub fn current(&self) -> &'a QueryPathNode<'a> {
        self.0
    }
}

impl<'a> Iterator for Parents<'a> {
    type Item = &'a QueryPathNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let parent = self.0.parent;
        if let Some(parent) = parent {
            self.0 = parent;
        }
        parent
    }
}

impl<'a> std::iter::FusedIterator for Parents<'a> {}

/// The unique id of the current resolution.
#[derive(Debug, Clone, Copy)]
pub struct ResolveId {
    /// The unique ID of the parent resolution.
    pub parent: Option<usize>,

    /// The current unique id.
    pub current: usize,
}

impl ResolveId {
    #[doc(hidden)]
    pub fn root() -> ResolveId {
        ResolveId {
            parent: None,
            current: 0,
        }
    }
}

impl Display for ResolveId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(parent) = self.parent {
            write!(f, "{}:{}", parent, self.current)
        } else {
            write!(f, "{}", self.current)
        }
    }
}

/// Query context.
///
/// **This type is not stable and should not be used directly.**
#[derive(Clone)]
pub struct ContextBase<'a, T> {
    /// The current path node being resolved.
    pub path_node: Option<QueryPathNode<'a>>,
    pub(crate) resolve_id: ResolveId,
    pub(crate) inc_resolve_id: &'a AtomicUsize,
    #[doc(hidden)]
    pub item: T,
    #[doc(hidden)]
    pub schema_env: &'a SchemaEnv,
    #[doc(hidden)]
    pub query_env: &'a QueryEnv,
}

#[doc(hidden)]
pub struct QueryEnvInner {
    pub extensions: Extensions,
    pub variables: Variables,
    pub operation: Positioned<OperationDefinition>,
    pub fragments: HashMap<Name, Positioned<FragmentDefinition>>,
    pub uploads: Vec<UploadValue>,
    pub ctx_data: Arc<Data>,
    pub http_headers: spin::Mutex<HeaderMap<String>>,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct QueryEnv(Arc<QueryEnvInner>);

impl Deref for QueryEnv {
    type Target = QueryEnvInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl QueryEnv {
    #[doc(hidden)]
    pub fn new(inner: QueryEnvInner) -> QueryEnv {
        QueryEnv(Arc::new(inner))
    }

    #[doc(hidden)]
    pub fn create_context<'a, T>(
        &'a self,
        schema_env: &'a SchemaEnv,
        path_node: Option<QueryPathNode<'a>>,
        item: T,
        resolve_id: ResolveId,
        inc_resolve_id: &'a AtomicUsize,
    ) -> ContextBase<'a, T> {
        ContextBase {
            path_node,
            resolve_id,
            inc_resolve_id,
            item,
            schema_env,
            query_env: self,
        }
    }
}

impl<'a, T> ContextBase<'a, T> {
    #[doc(hidden)]
    pub fn get_child_resolve_id(&self) -> ResolveId {
        let id = self
            .inc_resolve_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        ResolveId {
            parent: Some(self.resolve_id.current),
            current: id,
        }
    }

    #[doc(hidden)]
    pub fn with_field(
        &'a self,
        field: &'a Positioned<Field>,
    ) -> ContextBase<'a, &'a Positioned<Field>> {
        ContextBase {
            path_node: Some(QueryPathNode {
                parent: self.path_node.as_ref(),
                segment: QueryPathSegment::Name(&field.node.response_key().node),
            }),
            item: field,
            resolve_id: self.get_child_resolve_id(),
            inc_resolve_id: self.inc_resolve_id,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }

    #[doc(hidden)]
    pub fn with_selection_set(
        &self,
        selection_set: &'a Positioned<SelectionSet>,
    ) -> ContextBase<'a, &'a Positioned<SelectionSet>> {
        ContextBase {
            path_node: self.path_node,
            item: selection_set,
            resolve_id: self.resolve_id,
            inc_resolve_id: &self.inc_resolve_id,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }

    /// Gets the global data defined in the `Context` or `Schema`.
    ///
    /// If both `Schema` and `Query` have the same data type, the data in the `Query` is obtained.
    ///
    /// # Errors
    ///
    /// Returns a `Error` if the specified type data does not exist.
    pub fn data<D: Any + Send + Sync>(&self) -> Result<&'a D> {
        self.data_opt::<D>().ok_or_else(|| {
            Error::new(format!(
                "Data `{}` does not exist.",
                std::any::type_name::<D>()
            ))
        })
    }

    /// Gets the global data defined in the `Context` or `Schema`.
    ///
    /// # Panics
    ///
    /// It will panic if the specified data type does not exist.
    pub fn data_unchecked<D: Any + Send + Sync>(&self) -> &'a D {
        self.data_opt::<D>()
            .unwrap_or_else(|| panic!("Data `{}` does not exist.", std::any::type_name::<D>()))
    }

    /// Gets the global data defined in the `Context` or `Schema` or `None` if the specified type data does not exist.
    pub fn data_opt<D: Any + Send + Sync>(&self) -> Option<&'a D> {
        self.query_env
            .ctx_data
            .0
            .get(&TypeId::of::<D>())
            .or_else(|| self.schema_env.data.0.get(&TypeId::of::<D>()))
            .and_then(|d| d.downcast_ref::<D>())
    }

    /// Returns whether the HTTP header `key` is currently set on the response
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_graphql::*;
    /// use ::http::header::ACCESS_CONTROL_ALLOW_ORIGIN;
    ///
    /// struct Query;
    ///
    /// #[Object]
    /// impl Query {
    ///     async fn greet(&self, ctx: &Context<'_>) -> String {
    ///
    ///         let header_exists = ctx.http_header_contains("Access-Control-Allow-Origin");
    ///         assert!(!header_exists);
    ///
    ///         ctx.insert_http_header(ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    ///
    ///         let header_exists = ctx.http_header_contains("Access-Control-Allow-Origin");
    ///         assert!(header_exists);
    ///
    ///         String::from("Hello world")
    ///     }
    /// }
    /// ```
    pub fn http_header_contains(&self, key: impl AsHeaderName) -> bool {
        self.query_env.http_headers.lock().contains_key(key)
    }

    /// Sets a HTTP header to response.
    ///
    /// If the header was not currently set on the response, then `None` is returned.
    ///
    /// If the response already contained this header then the new value is associated with this key
    /// and __all the previous values are removed__, however only a the first previous
    /// value is returned.
    ///
    /// See [`http::HeaderMap`] for more details on the underlying implementation
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_graphql::*;
    /// use ::http::header::ACCESS_CONTROL_ALLOW_ORIGIN;
    ///
    /// struct Query;
    ///
    /// #[Object]
    /// impl Query {
    ///     async fn greet(&self, ctx: &Context<'_>) -> String {
    ///
    ///         // Headers can be inserted using the `http` constants
    ///         let was_in_headers = ctx.insert_http_header(ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    ///         assert_eq!(was_in_headers, None);
    ///
    ///         // They can also be inserted using &str
    ///         let was_in_headers = ctx.insert_http_header("Custom-Header", "1234");
    ///         assert_eq!(was_in_headers, None);
    ///
    ///         // If multiple headers with the same key are `inserted` then the most recent
    ///         // one overwrites the previous. If you want multiple headers for the same key, use
    ///         // `append_http_header` for subsequent headers
    ///         let was_in_headers = ctx.insert_http_header("Custom-Header", "Hello World");
    ///         assert_eq!(was_in_headers, Some("1234".to_string()));
    ///
    ///         String::from("Hello world")
    ///     }
    /// }
    /// ```
    pub fn insert_http_header(
        &self,
        name: impl IntoHeaderName,
        value: impl Into<String>,
    ) -> Option<String> {
        self.query_env
            .http_headers
            .lock()
            .insert(name, value.into())
    }

    /// Sets a HTTP header to response.
    ///
    /// If the header was not currently set on the response, then `false` is returned.
    ///
    /// If the response did have this header then the new value is appended to the end of the
    /// list of values currently associated with the key, however the key is not updated
    /// _(which is important for types that can be `==` without being identical)_.
    ///
    /// See [`http::HeaderMap`] for more details on the underlying implementation
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_graphql::*;
    /// use ::http::header::SET_COOKIE;
    ///
    /// struct Query;
    ///
    /// #[Object]
    /// impl Query {
    ///     async fn greet(&self, ctx: &Context<'_>) -> String {
    ///         // Insert the first instance of the header
    ///         ctx.insert_http_header(SET_COOKIE, "Chocolate Chip");
    ///
    ///         // Subsequent values should be appended
    ///         let header_already_exists = ctx.append_http_header("Set-Cookie", "Macadamia");
    ///         assert!(header_already_exists);
    ///
    ///         String::from("Hello world")
    ///     }
    /// }
    /// ```
    pub fn append_http_header(&self, name: impl IntoHeaderName, value: impl Into<String>) -> bool {
        self.query_env
            .http_headers
            .lock()
            .append(name, value.into())
    }

    fn var_value(&self, name: &str, pos: Pos) -> ServerResult<Value> {
        self.query_env
            .operation
            .node
            .variable_definitions
            .iter()
            .find(|def| def.node.name.node == name)
            .and_then(|def| {
                self.query_env
                    .variables
                    .0
                    .get(&def.node.name.node)
                    .or_else(|| def.node.default_value())
            })
            .cloned()
            .ok_or_else(|| ServerError::new(format!("Variable {} is not defined.", name)).at(pos))
    }

    fn resolve_input_value(&self, value: Positioned<InputValue>) -> ServerResult<Value> {
        let pos = value.pos;
        value
            .node
            .into_const_with(|name| self.var_value(&name, pos))
    }

    #[doc(hidden)]
    pub fn is_ifdef(&self, directives: &[Positioned<Directive>]) -> bool {
        directives
            .iter()
            .any(|directive| directive.node.name.node == "ifdef")
    }

    #[doc(hidden)]
    pub fn is_skip(&self, directives: &[Positioned<Directive>]) -> ServerResult<bool> {
        for directive in directives {
            let include = match &*directive.node.name.node {
                "skip" => false,
                "include" => true,
                _ => continue,
            };

            let condition_input = directive
                .node
                .get_argument("if")
                .ok_or_else(|| ServerError::new(format!(r#"Directive @{} requires argument `if` of type `Boolean!` but it was not provided."#, if include { "include" } else { "skip" })).at(directive.pos))?
                .clone();

            let pos = condition_input.pos;
            let condition_input = self.resolve_input_value(condition_input)?;

            if include
                != <bool as InputType>::parse(Some(condition_input))
                    .map_err(|e| e.into_server_error().at(pos))?
            {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

impl<'a> ContextBase<'a, &'a Positioned<SelectionSet>> {
    #[doc(hidden)]
    pub fn with_index(&'a self, idx: usize) -> ContextBase<'a, &'a Positioned<SelectionSet>> {
        ContextBase {
            path_node: Some(QueryPathNode {
                parent: self.path_node.as_ref(),
                segment: QueryPathSegment::Index(idx),
            }),
            item: self.item,
            resolve_id: self.get_child_resolve_id(),
            inc_resolve_id: self.inc_resolve_id,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }
}

impl<'a> ContextBase<'a, &'a Positioned<Field>> {
    #[doc(hidden)]
    pub fn param_value<T: InputType>(
        &self,
        name: &str,
        default: Option<fn() -> T>,
    ) -> ServerResult<T> {
        let value = self.item.node.get_argument(name).cloned();
        if value.is_none() {
            if let Some(default) = default {
                return Ok(default());
            }
        }
        let (pos, value) = match value {
            Some(value) => (value.pos, Some(self.resolve_input_value(value)?)),
            None => (Pos::default(), None),
        };
        InputType::parse(value).map_err(|e| e.into_server_error().at(pos))
    }

    /// Creates a uniform interface to inspect the forthcoming selections.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_graphql::*;
    ///
    /// #[derive(SimpleObject)]
    /// struct Detail {
    ///     c: i32,
    ///     d: i32,
    /// }
    ///
    /// #[derive(SimpleObject)]
    /// struct MyObj {
    ///     a: i32,
    ///     b: i32,
    ///     detail: Detail,
    /// }
    ///
    /// struct Query;
    ///
    /// #[Object]
    /// impl Query {
    ///     async fn obj(&self, ctx: &Context<'_>) -> MyObj {
    ///         if ctx.look_ahead().field("a").exists() {
    ///             // This is a query like `obj { a }`
    ///         } else if ctx.look_ahead().field("detail").field("c").exists() {
    ///             // This is a query like `obj { detail { c } }`
    ///         } else {
    ///             // This query doesn't have `a`
    ///         }
    ///         unimplemented!()
    ///     }
    /// }
    /// ```
    pub fn look_ahead(&self) -> Lookahead {
        Lookahead::new(&self.query_env.fragments, &self.item.node)
    }

    /// Get the current field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use async_graphql::*;
    ///
    /// #[derive(SimpleObject)]
    /// struct MyObj {
    ///     a: i32,
    ///     b: i32,
    ///     c: i32,
    /// }
    ///
    /// pub struct Query;
    ///
    /// #[Object]
    /// impl Query {
    ///     async fn obj(&self, ctx: &Context<'_>) -> MyObj {
    ///         let fields = ctx.field().selection_set().map(|field| field.name()).collect::<Vec<_>>();
    ///         assert_eq!(fields, vec!["a", "b", "c"]);
    ///         MyObj { a: 1, b: 2, c: 3 }
    ///     }
    /// }
    ///
    /// async_std::task::block_on(async move {
    ///     let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    ///     assert!(schema.execute("{ obj { a b c }}").await.is_ok());
    ///     assert!(schema.execute("{ obj { a ... { b c } }}").await.is_ok());
    ///     assert!(schema.execute("{ obj { a ... BC }} fragment BC on MyObj { b c }").await.is_ok());
    /// });
    ///
    /// ```
    pub fn field(&self) -> SelectionField<'a> {
        SelectionField {
            fragments: &self.query_env.fragments,
            field: &self.item.node,
        }
    }
}

/// Selection field.
#[derive(Clone, Copy)]
pub struct SelectionField<'a> {
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    field: &'a Field,
}

impl<'a> SelectionField<'a> {
    /// Get the name of this field.
    pub fn name(&self) -> &'a str {
        self.field.name.node.as_str()
    }

    /// Get all subfields of the current selection set.
    pub fn selection_set(&self) -> impl Iterator<Item = SelectionField<'a>> {
        SelectionFieldsIter {
            fragments: self.fragments,
            iter: vec![self.field.selection_set.node.items.iter()],
        }
    }
}

struct DebugSelectionSet<'a>(Vec<SelectionField<'a>>);

impl<'a> Debug for DebugSelectionSet<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.clone()).finish()
    }
}

impl<'a> Debug for SelectionField<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(self.name())
            .field("name", &self.name())
            .field(
                "selection_set",
                &DebugSelectionSet(self.selection_set().collect()),
            )
            .finish()
    }
}

struct SelectionFieldsIter<'a> {
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    iter: Vec<std::slice::Iter<'a, Positioned<Selection>>>,
}

impl<'a> Iterator for SelectionFieldsIter<'a> {
    type Item = SelectionField<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let it = self.iter.last_mut()?;
            match it.next() {
                Some(selection) => match &selection.node {
                    Selection::Field(field) => {
                        return Some(SelectionField {
                            fragments: self.fragments,
                            field: &field.node,
                        });
                    }
                    Selection::FragmentSpread(fragment_spread) => {
                        if let Some(fragment) =
                            self.fragments.get(&fragment_spread.node.fragment_name.node)
                        {
                            self.iter
                                .push(fragment.node.selection_set.node.items.iter());
                        }
                    }
                    Selection::InlineFragment(inline_fragment) => {
                        self.iter
                            .push(inline_fragment.node.selection_set.node.items.iter());
                    }
                },
                None => {
                    self.iter.pop();
                }
            }
        }
    }
}
