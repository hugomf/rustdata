use proc_macro::TokenStream;

mod entity;
mod projection_derive;
mod query_methods_derive;
mod sql_type_derive;

#[proc_macro_derive(Entity, attributes(entity))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    entity::expand_derive(input).into()
}

#[proc_macro_derive(SqlType, attributes(sql_type))]
pub fn derive_sql_type(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    sql_type_derive::expand_sql_type(input).into()
}

#[proc_macro_derive(Projection, attributes(projection))]
pub fn derive_projection(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    projection_derive::expand_projection(input).into()
}

#[proc_macro_derive(QueryMethods, attributes(entity))]
pub fn derive_query_methods(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    query_methods_derive::query_methods_derive(input).into()
}

#[cfg(test)]
mod tests;
