use crate::args::{self, CombineValidator, Validator};
use darling::FromMeta;
use proc_macro2::{Span, TokenStream, TokenTree};
use proc_macro_crate::crate_name;
use quote::quote;
use syn::{Attribute, Error, Expr, Ident, Lit, LitStr, Meta, NestedMeta};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeneratorError {
    #[error("{0}")]
    Syn(#[from] syn::Error),

    #[error("{0}")]
    Darling(#[from] darling::Error),
}

impl GeneratorError {
    pub fn write_errors(self) -> TokenStream {
        match self {
            GeneratorError::Syn(err) => err.to_compile_error(),
            GeneratorError::Darling(err) => err.write_errors(),
        }
    }
}

pub type GeneratorResult<T> = std::result::Result<T, GeneratorError>;

pub fn get_crate_name(internal: bool) -> TokenStream {
    if internal {
        quote! { crate }
    } else {
        let name = crate_name("async-graphql").unwrap_or_else(|_| "async_graphql".to_owned());
        TokenTree::from(Ident::new(&name, Span::call_site())).into()
    }
}

pub fn generate_validator(
    crate_name: &TokenStream,
    validator: &Validator,
) -> GeneratorResult<TokenStream> {
    Ok(match validator {
        Validator::Combine {
            combination,
            combination_span,
            validators,
        } => {
            let combination = Ident::new(
                match combination {
                    CombineValidator::And => "And",
                    CombineValidator::Or => "Or",
                },
                *combination_span,
            );

            validators
                .iter()
                .map(|validator| generate_validator(crate_name, validator))
                .try_fold(None, |acc, item| -> GeneratorResult<_> {
                    let item = item?;
                    Ok(Some(match acc {
                        Some(prev) => quote!(#crate_name::validators::#combination(#prev, #item)),
                        None => item,
                    }))
                })?
                .ok_or_else(|| {
                    syn::Error::new(*combination_span, "at least one validator is required")
                })?
        }
        Validator::Single(single) => {
            let path = &single.path;

            let constructor = if single.constructor_args.is_empty() {
                quote!(<#path as ::std::default::Default>::default())
            } else {
                let constructor_args: TokenStream = single
                    .constructor_args
                    .iter()
                    .map(|arg| {
                        Ok({
                            let arg: TokenStream = arg.parse()?;
                            quote!(#arg,)
                        })
                    })
                    .collect::<syn::Result<_>>()?;

                quote!(#path::new(#constructor_args))
            };

            let methods: TokenStream = single
                .methods
                .iter()
                .map(|method| {
                    Ok({
                        let name = &method.name;
                        let args: TokenStream = method
                            .args
                            .iter()
                            .map(|arg| {
                                Ok({
                                    let arg: TokenStream = arg.parse()?;
                                    quote!(#arg,)
                                })
                            })
                            .collect::<syn::Result<_>>()?;
                        quote!(.#name(#args))
                    })
                })
                .collect::<syn::Result<_>>()?;

            quote!(#constructor #methods)
        }
    })
}

pub fn generate_guards(
    crate_name: &TokenStream,
    args: &Meta,
) -> GeneratorResult<Option<TokenStream>> {
    match args {
        Meta::List(args) => {
            let mut guards = None;
            for item in &args.nested {
                if let NestedMeta::Meta(Meta::List(ls)) = item {
                    let ty = &ls.path;
                    let mut params = Vec::new();
                    for attr in &ls.nested {
                        if let NestedMeta::Meta(Meta::NameValue(nv)) = attr {
                            let name = &nv.path;
                            if let Lit::Str(value) = &nv.lit {
                                let value_str = value.value();
                                if value_str.starts_with('@') {
                                    let getter_name = get_param_getter_ident(&value_str[1..]);
                                    params.push(quote! { #name: #getter_name()? });
                                } else {
                                    let expr = syn::parse_str::<Expr>(&value_str)?;
                                    params.push(quote! { #name: (#expr).into() });
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.lit,
                                    "Value must be string literal",
                                )
                                .into());
                            }
                        } else {
                            return Err(
                                Error::new_spanned(attr, "Invalid property for guard").into()
                            );
                        }
                    }
                    let guard = quote! { #ty { #(#params),* } };
                    if guards.is_none() {
                        guards = Some(guard);
                    } else {
                        guards =
                            Some(quote! { #crate_name::guard::GuardExt::and(#guard, #guards) });
                    }
                } else {
                    return Err(Error::new_spanned(item, "Invalid guard").into());
                }
            }
            Ok(guards)
        }
        _ => Err(Error::new_spanned(args, "Invalid guards").into()),
    }
}

pub fn generate_post_guards(
    crate_name: &TokenStream,
    args: &Meta,
) -> GeneratorResult<Option<TokenStream>> {
    match args {
        Meta::List(args) => {
            let mut guards = None;
            for item in &args.nested {
                if let NestedMeta::Meta(Meta::List(ls)) = item {
                    let ty = &ls.path;
                    let mut params = Vec::new();
                    for attr in &ls.nested {
                        if let NestedMeta::Meta(Meta::NameValue(nv)) = attr {
                            let name = &nv.path;
                            if let Lit::Str(value) = &nv.lit {
                                let value_str = value.value();
                                if value_str.starts_with('@') {
                                    let getter_name = get_param_getter_ident(&value_str[1..]);
                                    params.push(quote! { #name: #getter_name()? });
                                } else {
                                    let expr = syn::parse_str::<Expr>(&value_str)?;
                                    params.push(quote! { #name: (#expr).into() });
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.lit,
                                    "Value must be string literal",
                                )
                                .into());
                            }
                        } else {
                            return Err(
                                Error::new_spanned(attr, "Invalid property for guard").into()
                            );
                        }
                    }
                    let guard = quote! { #ty { #(#params),* } };
                    if guards.is_none() {
                        guards = Some(guard);
                    } else {
                        guards =
                            Some(quote! { #crate_name::guard::PostGuardExt::and(#guard, #guards) });
                    }
                } else {
                    return Err(Error::new_spanned(item, "Invalid guard").into());
                }
            }
            Ok(guards)
        }
        _ => Err(Error::new_spanned(args, "Invalid guards").into()),
    }
}

pub fn get_rustdoc(attrs: &[Attribute]) -> GeneratorResult<Option<String>> {
    let mut full_docs = String::new();
    for attr in attrs {
        match attr.parse_meta()? {
            Meta::NameValue(nv) if nv.path.is_ident("doc") => {
                if let Lit::Str(doc) = nv.lit {
                    let doc = doc.value();
                    let doc_str = doc.trim();
                    if !full_docs.is_empty() {
                        full_docs += "\n";
                    }
                    full_docs += doc_str;
                }
            }
            _ => {}
        }
    }
    Ok(if full_docs.is_empty() {
        None
    } else {
        Some(full_docs)
    })
}

fn generate_default_value(lit: &Lit) -> GeneratorResult<TokenStream> {
    match lit {
        Lit::Str(value) =>{
            let value = value.value();
            Ok(quote!({ #value.to_string() }))
        }
        Lit::Int(value) => {
            let value = value.base10_parse::<i32>()?;
            Ok(quote!({ #value as i32 }))
        }
        Lit::Float(value) => {
            let value = value.base10_parse::<f64>()?;
            Ok(quote!({ #value as f64 }))
        }
        Lit::Bool(value) => {
            let value = value.value;
            Ok(quote!({ #value }))
        }
        _ => Err(Error::new_spanned(
            lit,
            "The default value type only be string, integer, float and boolean, other types should use default_with",
        ).into()),
    }
}

fn generate_default_with(lit: &LitStr) -> GeneratorResult<TokenStream> {
    let str = lit.value();
    let tokens: TokenStream = str
        .parse()
        .map_err(|err| GeneratorError::Syn(syn::Error::from(err)))?;
    Ok(quote! { (#tokens) })
}

pub fn generate_default(
    default: &Option<args::DefaultValue>,
    default_with: &Option<LitStr>,
) -> GeneratorResult<Option<TokenStream>> {
    match (default, default_with) {
        (Some(args::DefaultValue::Default), _) => Ok(Some(quote! { Default::default() })),
        (Some(args::DefaultValue::Value(lit)), _) => Ok(Some(generate_default_value(lit)?)),
        (None, Some(lit)) => Ok(Some(generate_default_with(lit)?)),
        (None, None) => Ok(None),
    }
}

pub fn get_param_getter_ident(name: &str) -> Ident {
    Ident::new(&format!("__{}_getter", name), Span::call_site())
}

pub fn get_cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path.segments.is_empty() && attr.path.segments[0].ident == "cfg")
        .cloned()
        .collect()
}

pub fn parse_graphql_attrs<T: FromMeta>(attrs: &[Attribute]) -> GeneratorResult<Option<T>> {
    for attr in attrs {
        if attr.path.is_ident("graphql") {
            let meta = attr.parse_meta()?;
            return Ok(Some(T::from_meta(&meta)?));
        }
    }
    Ok(None)
}

pub fn remove_graphql_attrs(attrs: &mut Vec<Attribute>) {
    if let Some((idx, _)) = attrs
        .iter()
        .enumerate()
        .find(|(_, a)| a.path.is_ident("graphql"))
    {
        attrs.remove(idx);
    }
}
