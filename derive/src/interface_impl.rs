use proc_macro::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::{Block, Error, FnArg, ImplItem, ItemImpl, Pat, ReturnType, Type, TypeReference};

use crate::args;
use crate::args::{RenameRuleExt, RenameTarget};
use crate::output_type::OutputType;
use crate::utils::{
    generate_default, get_cfg_attrs, get_crate_name, get_param_getter_ident, get_rustdoc,
    get_type_path_and_name, parse_graphql_attrs, remove_graphql_attrs, visible_fn, GeneratorResult,
};

pub fn generate(
    interface_args: &args::InterfaceImpl,
    item_impl: &mut ItemImpl,
) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_name(interface_args.internal);
    let (self_ty, _) = get_type_path_and_name(item_impl.self_ty.as_ref())?;
    let generics = &item_impl.generics;
    let where_clause = &item_impl.generics.where_clause;

    let mut resolvers = Vec::new();
    let mut schema_fields = Vec::new();

    for item in &mut item_impl.items {
        if let ImplItem::Method(method) = item {
            let method_args: args::InterfaceImplField =
                parse_graphql_attrs(&method.attrs)?.unwrap_or_default();

            if method.sig.asyncness.is_none() {
                return Err(Error::new_spanned(&method, "Must be asynchronous").into());
            }

            let field_name = method_args.name.clone().unwrap_or_else(|| {
                interface_args
                    .rename_fields
                    .rename(method.sig.ident.unraw().to_string(), RenameTarget::Field)
            });
            let field_desc = get_rustdoc(&method.attrs)?
                .map(|s| quote! { ::std::option::Option::Some(#s) })
                .unwrap_or_else(|| quote! {::std::option::Option::None});
            let field_deprecation = method_args
                .deprecation
                .as_ref()
                .map(|s| quote! { ::std::option::Option::Some(#s) })
                .unwrap_or_else(|| quote! {::std::option::Option::None});
            let external = method_args.external;
            let requires = match &method_args.requires {
                Some(requires) => quote! { ::std::option::Option::Some(#requires) },
                None => quote! { ::std::option::Option::None },
            };
            let provides = match &method_args.provides {
                Some(provides) => quote! { ::std::option::Option::Some(#provides) },
                None => quote! { ::std::option::Option::None },
            };
            let ty = match &method.sig.output {
                ReturnType::Type(_, ty) => OutputType::parse(ty)?,
                ReturnType::Default => {
                    return Err(Error::new_spanned(&method.sig.output, "Missing type").into())
                }
            };
            let cfg_attrs = get_cfg_attrs(&method.attrs);

            let mut create_ctx = true;
            let mut args = Vec::new();

            for (idx, arg) in method.sig.inputs.iter_mut().enumerate() {
                if let FnArg::Receiver(receiver) = arg {
                    if idx != 0 {
                        return Err(Error::new_spanned(
                            receiver,
                            "The self receiver must be the first parameter.",
                        )
                        .into());
                    }
                } else if let FnArg::Typed(pat) = arg {
                    if idx == 0 {
                        return Err(Error::new_spanned(
                            pat,
                            "The self receiver must be the first parameter.",
                        )
                        .into());
                    }

                    match (&*pat.pat, &*pat.ty) {
                        (Pat::Ident(arg_ident), Type::Path(arg_ty)) => {
                            args.push((
                                arg_ident.clone(),
                                arg_ty.clone(),
                                parse_graphql_attrs::<args::InterfaceImplFieldArgument>(
                                    &pat.attrs,
                                )?
                                .unwrap_or_default(),
                            ));
                            remove_graphql_attrs(&mut pat.attrs);
                        }
                        (arg, Type::Reference(TypeReference { elem, .. })) => {
                            if let Type::Path(path) = elem.as_ref() {
                                if idx != 1 || path.path.segments.last().unwrap().ident != "Context"
                                {
                                    return Err(Error::new_spanned(
                                        arg,
                                        "Only types that implement `InputType` can be used as input arguments.",
                                    )
                                        .into());
                                }

                                create_ctx = false;
                            }
                        }
                        _ => return Err(Error::new_spanned(arg, "Invalid argument type.").into()),
                    }
                }
            }

            if create_ctx {
                let arg = syn::parse2::<FnArg>(quote! { _: &#crate_name::Context<'_> }).unwrap();
                method.sig.inputs.insert(1, arg);
            }

            let mut schema_args = Vec::new();
            let mut use_params = Vec::new();
            let mut get_params = Vec::new();

            for (
                ident,
                ty,
                args::InterfaceImplFieldArgument {
                    name,
                    desc,
                    default,
                    default_with,
                    visible,
                },
            ) in args
            {
                let name = name.clone().unwrap_or_else(|| {
                    interface_args
                        .rename_args
                        .rename(ident.ident.unraw().to_string(), RenameTarget::Argument)
                });
                let desc = desc
                    .as_ref()
                    .map(|s| quote! {::std::option::Option::Some(#s)})
                    .unwrap_or_else(|| quote! {::std::option::Option::None});
                let default = generate_default(&default, &default_with)?;
                let schema_default = default
                    .as_ref()
                    .map(|value| {
                        quote! {
                            ::std::option::Option::Some(::std::string::ToString::to_string(
                                &<#ty as #crate_name::InputType>::to_value(&#value)
                            ))
                        }
                    })
                    .unwrap_or_else(|| quote! {::std::option::Option::None});
                let visible = visible_fn(&visible);
                schema_args.push(quote! {
                    args.insert(#name, #crate_name::registry::MetaInputValue {
                        name: #name,
                        description: #desc,
                        ty: <#ty as #crate_name::Type>::create_type_info(registry),
                        default_value: #schema_default,
                        validator: ::std::option::Option::None,
                        visible: #visible,
                    });
                });

                let param_ident = &ident.ident;
                use_params.push(quote! { #param_ident });

                let default = match default {
                    Some(default) => {
                        quote! { ::std::option::Option::Some(|| -> #ty { #default }) }
                    }
                    None => quote! { ::std::option::Option::None },
                };
                let param_getter_name = get_param_getter_ident(&ident.ident.to_string());
                get_params.push(quote! {
                    #[allow(non_snake_case)]
                    let #param_getter_name = || -> #crate_name::ServerResult<#ty> { ctx.param_value(#name, #default) };
                    #[allow(non_snake_case)]
                    let #ident: #ty = #param_getter_name()?;
                });
            }

            let schema_ty = ty.value_type();
            let visible = visible_fn(&method_args.visible);

            schema_fields.push(quote! {
                #(#cfg_attrs)*
                fields.insert(::std::borrow::ToOwned::to_owned(#field_name), #crate_name::registry::MetaField {
                    name: ::std::borrow::ToOwned::to_owned(#field_name),
                    description: #field_desc,
                    args: {
                        let mut args = #crate_name::indexmap::IndexMap::new();
                        #(#schema_args)*
                        args
                    },
                    ty: <#schema_ty as #crate_name::Type>::create_type_info(registry),
                    deprecation: #field_deprecation,
                    cache_control: ::std::default::Default::default(),
                    external: #external,
                    provides: #provides,
                    requires: #requires,
                    visible: #visible,
                });
            });

            let field_ident = &method.sig.ident;
            if let OutputType::Value(inner_ty) = &ty {
                let block = &method.block;
                let new_block = quote!({
                    {
                        let value:#inner_ty = async move #block.await;
                        ::std::result::Result::Ok(value)
                    }
                });
                method.block = syn::parse2::<Block>(new_block).expect("invalid block");
                method.sig.output =
                    syn::parse2::<ReturnType>(quote! { -> #crate_name::Result<#inner_ty> })
                        .expect("invalid result type");
            }

            let resolve_obj = quote! {
                {
                    let res = self.#field_ident(ctx, #(#use_params),*).await;
                    res.map_err(|err| err.into_server_error().at(ctx.item.pos))?
                }
            };

            resolvers.push(quote! {
                #(#cfg_attrs)*
                if ctx.item.node.name.node == #field_name {
                    #(#get_params)*
                    let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                    let res = #resolve_obj;
                    return #crate_name::OutputType::resolve(&res, &ctx_obj, ctx.item).await.map(::std::option::Option::Some);
                }
            });

            remove_graphql_attrs(&mut method.attrs);
        }
    }

    let expanded = quote! {
        #crate_name::static_assertions::assert_impl_one!(#self_ty: #crate_name::InterfaceDefinition);

        #item_impl

        #[allow(clippy::all, clippy::pedantic)]
        impl #generics #crate_name::Type for #self_ty #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                <#self_ty as #crate_name::InterfaceDefinition>::type_name()
            }

            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                let mut fields = #crate_name::indexmap::IndexMap::new();
                #(#schema_fields)*
                <#self_ty as #crate_name::InterfaceDefinition>::create_type_info(registry, fields)
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        #[#crate_name::async_trait::async_trait]
        impl#generics #crate_name::resolver_utils::ContainerType for #self_ty #where_clause {
            async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                #(#resolvers)*
                ::std::result::Result::Ok(::std::option::Option::None)
            }

            fn collect_all_fields<'__life>(&'__life self, ctx: &#crate_name::ContextSelectionSet<'__life>, fields: &mut #crate_name::resolver_utils::Fields<'__life>) -> #crate_name::ServerResult<()> {
                <#self_ty as #crate_name::InterfaceDefinition>::collect_all_fields(self, ctx, fields)
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        #[#crate_name::async_trait::async_trait]
        impl #generics #crate_name::OutputType for #self_ty #where_clause {
            async fn resolve(&self, ctx: &#crate_name::ContextSelectionSet<'_>, _field: &#crate_name::Positioned<#crate_name::parser::types::Field>) -> #crate_name::ServerResult<#crate_name::Value> {
                #crate_name::resolver_utils::resolve_container(ctx, self).await
            }
        }

        impl #generics #crate_name::InterfaceType for #self_ty #where_clause {}
    };
    Ok(expanded.into())
}
