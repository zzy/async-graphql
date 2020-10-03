use crate::args;
use crate::utils::{get_crate_name, get_rustdoc, GeneratorResult};
use proc_macro::TokenStream;
use quote::quote;

pub fn generate(scalar_args: &args::Scalar) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_name(scalar_args.internal);
    let ident = &scalar_args.ident;
    let (impl_generics, type_generics, where_clause) = scalar_args.generics.split_for_impl();
    let gql_typename = scalar_args
        .name
        .clone()
        .unwrap_or_else(|| ident.to_string());

    let desc = get_rustdoc(&scalar_args.attrs)?
        .map(|s| quote! { Some(#s) })
        .unwrap_or_else(|| quote! {None});

    let expanded = quote! {
        impl #impl_generics #crate_name::ScalarType for #ident #type_generics #where_clause {}

        #[allow(clippy::all, clippy::pedantic)]
        impl #impl_generics #crate_name::Type for #ident #type_generics #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(#gql_typename)
            }

            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                registry.create_type::<Self, _>(|_| #crate_name::registry::MetaType::Scalar {
                    name: ::std::string::ToString::to_string(&#gql_typename),
                    description: #desc,
                })
            }
        }

        #[allow(clippy::all, clippy::pedantic)]
        impl #impl_generics #crate_name::InputValueType for #ident #type_generics #where_clause {}

        #[allow(clippy::all, clippy::pedantic)]
        #[#crate_name::async_trait::async_trait]
        impl #impl_generics #crate_name::OutputValueType for #ident #type_generics #where_clause {
            async fn resolve(
                &self,
                ctx: &#crate_name::ContextSelectionSet<'_>,
                _field: &#crate_name::Positioned<#crate_name::parser::types::Field>
            ) -> #crate_name::ServerResult<#crate_name::serde_json::Value> {
                #crate_name::serde_json::to_value(self).map_err(|e| #crate_name::ServerError::new(::std::string::ToString::to_string(&e)).at(ctx.item.pos))
            }
        }
    };

    Ok(expanded.into())
}
