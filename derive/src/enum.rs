use crate::args;
use crate::utils::{get_crate_name, get_rustdoc, GeneratorResult};
use darling::ast::Data;
use inflector::Inflector;
use proc_macro::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::Error;

pub fn generate(enum_args: &args::Enum) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_name(enum_args.internal);
    let ident = &enum_args.ident;
    let e = match &enum_args.data {
        Data::Enum(e) => e,
        _ => return Err(Error::new_spanned(ident, "Enum can only be applied to an enum.").into()),
    };

    let gql_typename = enum_args.name.clone().unwrap_or_else(|| ident.to_string());

    let desc = get_rustdoc(&enum_args.attrs)?
        .map(|s| quote! { Some(#s) })
        .unwrap_or_else(|| quote! {None});

    let mut enum_items = Vec::new();
    let mut de_variant_arms = proc_macro2::TokenStream::new();
    let mut ser_variant_arms = proc_macro2::TokenStream::new();
    let mut variants = proc_macro2::TokenStream::new();
    let mut schema_enum_items = Vec::new();

    for (i, variant) in e.iter().enumerate() {
        if !variant.fields.is_empty() {
            return Err(Error::new_spanned(
                &variant.ident,
                format!(
                    "Invalid enum variant {}.\nGraphQL enums may only contain unit variants.",
                    variant.ident
                ),
            )
            .into());
        }

        let item_ident = &variant.ident;
        let gql_item_name = variant
            .name
            .clone()
            .take()
            .unwrap_or_else(|| variant.ident.unraw().to_string().to_screaming_snake_case());
        let item_deprecation = variant
            .deprecation
            .as_ref()
            .map(|s| quote! { Some(#s) })
            .unwrap_or_else(|| quote! {None});
        let item_desc = get_rustdoc(&variant.attrs)?
            .map(|s| quote! { Some(#s) })
            .unwrap_or_else(|| quote! {None});

        enum_items.push(item_ident);
        de_variant_arms.extend(quote! {
            #gql_item_name => #ident::#item_ident,
        });
        ser_variant_arms.extend(quote! {
            #ident::#item_ident => (#i, #gql_item_name),
        });
        variants.extend(quote!(#gql_item_name,));
        schema_enum_items.push(quote! {
            enum_items.insert(#gql_item_name, #crate_name::registry::MetaEnumValue {
                name: #gql_item_name,
                description: #item_desc,
                deprecation: #item_deprecation,
            });
        });
    }

    let remote_conversion = if let Some(remote) = &enum_args.remote {
        let remote_ty = if let Ok(ty) = syn::parse_str::<syn::Type>(remote) {
            ty
        } else {
            return Err(
                Error::new_spanned(remote, format!("Invalid remote type: '{}'", remote)).into(),
            );
        };

        let local_to_remote_items = enum_items.iter().map(|item| {
            quote! {
                #ident::#item => #remote_ty::#item,
            }
        });
        let remote_to_local_items = enum_items.iter().map(|item| {
            quote! {
                #remote_ty::#item => #ident::#item,
            }
        });
        Some(quote! {
            impl ::std::convert::From<#ident> for #remote_ty {
                fn from(value: #ident) -> Self {
                    match value {
                        #(#local_to_remote_items)*
                    }
                }
            }

            impl ::std::convert::From<#remote_ty> for #ident {
                fn from(value: #remote_ty) -> Self {
                    match value {
                        #(#remote_to_local_items)*
                    }
                }
            }
        })
    } else {
        None
    };

    let expanded = quote! {
        #[allow(clippy::all, clippy::pedantic)]
        impl<'de> #crate_name::serde::Deserialize<'de> for #ident {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<
                Self,
                <D as #crate_name::serde::Deserializer<'de>>::Error
            >
            where
                D: #crate_name::serde::Deserializer<'de>,
            {
                const VARIANTS: &[&str] = [#variants];

                struct DeserializeVariant(#ident);
                impl<'de> #crate_name::serde::Deserialize<'de> for DeserializeVariant {
                    fn deserialize<D>(deserializer: D) -> ::std::result::Result<
                        Self,
                        <D as #crate_name::serde::Deserializer<'de>>::Error
                    >
                    where
                        D: #crate_name::serde::Deserializer<'de>,
                    {
                        struct VariantVisitor;
                        impl<'de> #crate_name::serde::de::Visitor<'de> for VariantVisitor {
                            type Value = #ident;

                            fn expecting(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                                f.write_str("variant identifier")
                            }
                            fn visit_str<E: #crate_name::serde::de::Error>(
                                self,
                                value: &::std::primitive::str
                            ) -> ::std::result::Result<Self::Value, E> {
                                ::std::result::Result::Ok(match value {
                                    #de_variant_arms
                                    _ => return ::std::result::Result::Err(
                                        <E as #crate_name::serde::de::Error>::unknown_variant(value, VARIANTS)
                                    ),
                                })
                            }
                        }
                        #crate_name::serde::Deserializer::deserialize_identifier(
                            deserializer,
                            VariantVisitor
                        )
                    }
                }

                struct Visitor;
                impl<'de> #crate_name::serde::de::Visitor<'de> for Visitor {
                    type Value = #ident;

                    fn expecting(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                        f.write_str(::std::concat!("enum \"", #gql_typename, "\""))
                    }
                    fn visit_enum<A>(self, data: A) -> ::std::result::Result<Self::Value, A::Error>
                    where
                        A: #crate_name::serde::de::EnumAccess<'de>,
                    {
                        let (variant, access) = #crate_name::serde::de::EnumAccess::variant(data)?;
                        #crate_name::serde::de::VariantAcesss::unit_variant(access)?;
                        ::std::result::Result::Ok(variant)
                    }
                }

                #crate_name::serde::Deserializer::deserialize_enum(
                    deserializer,
                    #gql_typename,
                    VARIANTS,
                    Visitor,
                )
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        impl #crate_name::serde::Serialize for #ident {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #crate_name::serde::Serializer,
            {
                let (i, name): (u32, &'static str) = match *self { #ser_variant_arms };
                #crate_name::serde::Serializer::serialize_unit_variant(
                    serializer,
                    #gql_typename,
                    i,
                    name,
                )
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        impl #crate_name::Type for #ident {
            fn type_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(#gql_typename)
            }

            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> String {
                registry.create_type::<Self, _>(|registry| {
                    #crate_name::registry::MetaType::Enum {
                        name: #gql_typename.to_string(),
                        description: #desc,
                        enum_values: {
                            let mut enum_items = #crate_name::indexmap::IndexMap::new();
                            #(#schema_enum_items)*
                            enum_items
                        },
                    }
                })
            }
        }

        impl #crate_name::InputValueType for #ident {}

        #[#crate_name::async_trait::async_trait]
        impl #crate_name::OutputValueType for #ident {
            async fn resolve(
                &self,
                _: &#crate_name::ContextSelectionSet<'_>,
                _field: &#crate_name::Positioned<#crate_name::parser::types::Field>,
            ) -> #crate_name::ServerResult<#crate_name::serde_json::Value> {
                ::std::result::Result::Ok(#crate_name::serde_json::to_value(self).unwrap())
            }
        }

        #remote_conversion
    };
    Ok(expanded.into())
}
