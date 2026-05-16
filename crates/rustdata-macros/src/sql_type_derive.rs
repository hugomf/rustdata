use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(sql_type))]
struct SqlTypeAttrs {
    delegate: String,
}

pub fn expand_sql_type(input: DeriveInput) -> TokenStream {
    let attrs = SqlTypeAttrs::from_derive_input(&input)
        .expect("#[sql_type(delegate = \"...\")] is required");

    let name = &input.ident;
    let delegate: TokenStream = attrs.delegate.parse().expect("invalid delegate type");

    quote! {
        impl ::rustdata_core::sql_type::SqlBind for #name {
            fn sql_bind<'q, DB, B>(
                q: ::rustdata_core::bind::QueryBuilder<'q, DB>,
                v: &'q Self,
            ) -> ::rustdata_core::bind::QueryBuilder<'q, DB>
            where
                DB: sqlx::Database,
                B: ::rustdata_core::bind::BindAdapter<DB>,
            {
                <#delegate as ::rustdata_core::sql_type::SqlBind>
                    ::sql_bind::<DB, B>(q, &v.0)
            }
        }

        impl ::rustdata_core::sql_type::SqlExtract for #name {
            fn sql_extract<E: ::rustdata_core::descriptor::RowExtractor>(
                ext: &E,
                row: &E::Row,
                col: &str,
            ) -> Result<Self, ::rustdata_core::error::RepositoryError> {
                Ok(#name(
                    <#delegate as ::rustdata_core::sql_type::SqlExtract>
                        ::sql_extract(ext, row, col)?
                ))
            }
        }

        impl ::rustdata_core::sql_type::SqlBind for Option<#name> {
            fn sql_bind<'q, DB, B>(
                q: ::rustdata_core::bind::QueryBuilder<'q, DB>,
                v: &'q Self,
            ) -> ::rustdata_core::bind::QueryBuilder<'q, DB>
            where
                DB: sqlx::Database,
                B: ::rustdata_core::bind::BindAdapter<DB>,
            {
                if let Some(inner) = v {
                    <#delegate as ::rustdata_core::sql_type::SqlBind>
                        ::sql_bind::<DB, B>(q, &inner.0)
                } else {
                    B::bind_opt_str(q, None)
                }
            }
        }

        impl ::rustdata_core::sql_type::SqlExtract for Option<#name> {
            fn sql_extract<E: ::rustdata_core::descriptor::RowExtractor>(
                ext: &E,
                row: &E::Row,
                col: &str,
            ) -> Result<Self, ::rustdata_core::error::RepositoryError> {
                <Option<#delegate> as ::rustdata_core::sql_type::SqlExtract>
                    ::sql_extract(ext, row, col)
                    .map(|opt| opt.map(#name))
            }
        }
    }
}
