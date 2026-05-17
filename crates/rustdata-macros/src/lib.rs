use proc_macro::TokenStream;

mod entity;
mod include_migrations;
mod projection_derive;
mod query_methods_derive;
mod sql_type_derive;

#[proc_macro_derive(Entity, attributes(entity))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    entity::expand_derive(input).into()
}

/// Compile-time glob of migration SQL files.
///
/// Accepts a string literal path relative to the crate root and expands to a
/// `&'static [(&'static str, &'static str)]` of `(stem, sql)` pairs sorted
/// by numeric version prefix.
///
/// **Do not call this directly** — use `rustdata_migrations::migrate!` instead.
#[proc_macro]
pub fn include_migrations(input: TokenStream) -> TokenStream {
    include_migrations::expand_include_migrations(input)
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
