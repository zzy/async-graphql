use darling::ast::{Data, Fields};
use darling::util::Ignored;
use darling::{FromDeriveInput, FromField, FromMeta, FromVariant};
use syn::{Attribute, Generics, Ident, Lit, LitStr, Meta, NestedMeta, Path, Type, Visibility};
use proc_macro2::Span;

#[derive(FromMeta)]
#[darling(default)]
pub struct CacheControl {
    public: bool,
    private: bool,
    pub max_age: usize,
}

impl Default for CacheControl {
    fn default() -> Self {
        Self {
            public: true,
            private: false,
            max_age: 0,
        }
    }
}

impl CacheControl {
    pub fn is_public(&self) -> bool {
        !self.private && self.public
    }
}

#[derive(Debug)]
pub enum DefaultValue {
    Default,
    Value(Lit),
}

impl FromMeta for DefaultValue {
    fn from_word() -> darling::Result<Self> {
        Ok(DefaultValue::Default)
    }

    fn from_value(value: &Lit) -> darling::Result<Self> {
        Ok(DefaultValue::Value(value.clone()))
    }
}

#[derive(FromField)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct SimpleObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,

    #[darling(default)]
    pub skip: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub deprecation: Option<String>,
    #[darling(default)]
    pub owned: bool,
    #[darling(default)]
    pub cache_control: CacheControl,
    #[darling(default)]
    pub external: bool,
    #[darling(default)]
    pub provides: Option<String>,
    #[darling(default)]
    pub requires: Option<String>,
    #[darling(default)]
    pub guard: Option<Meta>,
    #[darling(default)]
    pub post_guard: Option<Meta>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct SimpleObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, SimpleObjectField>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub cache_control: CacheControl,
    #[darling(default)]
    pub extends: bool,
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Argument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub default_with: Option<LitStr>,
    pub validator: Option<Validator>,
    pub key: bool, // for entity
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Object {
    pub internal: bool,
    pub name: Option<String>,
    pub cache_control: CacheControl,
    pub extends: bool,
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct ObjectField {
    pub skip: bool,
    pub entity: bool,
    pub name: Option<String>,
    pub deprecation: Option<String>,
    pub cache_control: CacheControl,
    pub external: bool,
    pub provides: Option<String>,
    pub requires: Option<String>,
    pub guard: Option<Meta>,
    pub post_guard: Option<Meta>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Enum {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<EnumItem, Ignored>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub remote: Option<String>,
}

#[derive(FromVariant)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct EnumItem {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub fields: Fields<Ignored>,

    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub deprecation: Option<String>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Union {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<UnionItem, Ignored>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
}

#[derive(FromVariant)]
#[darling(attributes(graphql))]
pub struct UnionItem {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,

    #[darling(default)]
    pub flatten: bool,
}

#[derive(FromField)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct InputObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,

    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub default: Option<DefaultValue>,
    #[darling(default)]
    pub default_with: Option<LitStr>,
    #[darling(default)]
    pub validator: Option<Validator>,
    #[darling(default)]
    pub flatten: bool,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct InputObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, InputObjectField>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
}

#[derive(FromMeta)]
pub struct InterfaceFieldArgument {
    pub name: String,
    #[darling(default)]
    pub desc: Option<String>,
    #[darling(rename = "type")]
    pub ty: LitStr,
    #[darling(default)]
    pub default: Option<DefaultValue>,
    #[darling(default)]
    pub default_with: Option<LitStr>,
}

#[derive(FromMeta)]
pub struct InterfaceField {
    pub name: String,
    #[darling(rename = "type")]
    pub ty: LitStr,
    #[darling(default)]
    pub method: Option<String>,
    #[darling(default)]
    pub desc: Option<String>,
    #[darling(default, multiple, rename = "arg")]
    pub args: Vec<InterfaceFieldArgument>,
    #[darling(default)]
    pub deprecation: Option<String>,
    #[darling(default)]
    pub external: bool,
    #[darling(default)]
    pub provides: Option<String>,
    #[darling(default)]
    pub requires: Option<String>,
}

#[derive(FromVariant)]
pub struct InterfaceMember {
    pub ident: Ident,
    pub fields: Fields<syn::Type>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Interface {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<InterfaceMember, Ignored>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default, multiple, rename = "field")]
    pub fields: Vec<InterfaceField>,
    #[darling(default)]
    pub extends: bool,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct Scalar {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct Subscription {
    pub internal: bool,
    pub name: Option<String>,
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct SubscriptionFieldArgument {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    pub default_with: Option<LitStr>,
    pub validator: Option<Validator>,
}

#[derive(FromMeta, Default)]
#[darling(default)]
pub struct SubscriptionField {
    pub skip: bool,
    pub name: Option<String>,
    pub deprecation: Option<String>,
    pub guard: Option<Meta>,
    pub post_guard: Option<Meta>,
}

#[derive(FromMeta, Default)]
#[darling(default, allow_unknown_fields)]
pub struct SubscriptionFieldWrapper {
    pub graphql: SubscriptionField,
}

#[derive(FromField)]
pub struct MergedObjectField {
    pub ident: Option<Ident>,
    pub ty: Type,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct MergedObject {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, MergedObjectField>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub cache_control: CacheControl,
    #[darling(default)]
    pub extends: bool,
}

#[derive(FromField)]
pub struct MergedSubscriptionField {
    pub ident: Option<Ident>,
    pub ty: Type,
}

#[derive(FromDeriveInput)]
#[darling(attributes(graphql), forward_attrs(doc))]
pub struct MergedSubscription {
    pub ident: Ident,
    pub generics: Generics,
    pub attrs: Vec<Attribute>,
    pub data: Data<Ignored, MergedSubscriptionField>,

    #[darling(default)]
    pub internal: bool,
    #[darling(default)]
    pub name: Option<String>,
}

pub enum Validator {
    Combine {
        combination: CombineValidator,
        combination_span: Span,
        validators: Vec<Validator>
    },
    Single(SingleValidator),
}

impl FromMeta for Validator {
    fn from_meta(meta: &Meta) -> darling::Result<Self> {
        match meta {
            Meta::List(list) => Some(list),
            _ => None,
        }
        .and_then(|list| {
            list.path
                .get_ident()
                .and_then(|i| {
                    if i == "and" {
                        Some((CombineValidator::And, i))
                    } else if i == "or" {
                        Some((CombineValidator::Or, i))
                    } else {
                        None
                    }
                })
                .map(|(combination, ident)| (combination, ident.span(), list))
        })
        .map_or_else(
            || Ok(Self::Single(SingleValidator::from_meta(meta)?)),
            |(combination, combination_span, list)| {
                Ok(Self::Combine {
                    combination,
                    combination_span,
                    validators: list.nested
                        .iter()
                        .map(Self::from_nested_meta)
                        .collect::<darling::Result<_>>()?,
                })
            },
        )
    }
}

pub enum CombineValidator {
    And,
    Or,
}

pub struct SingleValidator {
    pub path: Path,
    pub constructor_args: Vec<LitStr>,
    pub methods: Vec<ValidatorMethod>,
}

impl FromMeta for SingleValidator {
    fn from_meta(meta: &Meta) -> darling::Result<Self> {
        Ok(match meta {
            Meta::Path(path) => Self {
                path: path.clone(),
                constructor_args: Vec::new(),
                methods: Vec::new(),
            },
            Meta::NameValue(nv) => Self {
                path: nv.path.clone(),
                constructor_args: vec![LitStr::from_value(&nv.lit)?],
                methods: Vec::new(),
            },
            Meta::List(list) => {
                let mut nested = list.nested.iter().fuse();

                let constructor_args = nested
                    .by_ref()
                    .take_while(|nested| matches!(nested, NestedMeta::Lit(_)))
                    .map(|nested| match nested {
                        NestedMeta::Lit(lit) => LitStr::from_value(lit),
                        _ => unreachable!(),
                    })
                    .collect::<darling::Result<_>>()?;

                let methods = nested
                    .map(|nested| match nested {
                        NestedMeta::Meta(meta) => ValidatorMethod::from_meta(meta),
                        nested => Err(darling::Error::custom(
                            "validator constructor arguments must be before validator methods",
                        )
                        .with_span(nested)),
                    })
                    .collect::<darling::Result<_>>()?;

                Self {
                    path: list.path.clone(),
                    constructor_args,
                    methods,
                }
            }
        })
    }
}

pub struct ValidatorMethod {
    pub name: Ident,
    pub args: Vec<LitStr>,
}

impl FromMeta for ValidatorMethod {
    fn from_meta(meta: &Meta) -> darling::Result<Self> {
        let (path, args) = match meta {
            Meta::Path(path) => (path, Vec::new()),
            Meta::NameValue(nv) => (&nv.path, vec![LitStr::from_value(&nv.lit)?]),
            Meta::List(list) => (
                &list.path,
                list.nested
                    .iter()
                    .map(LitStr::from_nested_meta)
                    .collect::<darling::Result<_>>()?,
            ),
        };

        Ok(Self {
            name: path
                .get_ident()
                .ok_or_else(|| darling::Error::custom("expected method name").with_span(path))?
                .clone(),
            args,
        })
    }
}
