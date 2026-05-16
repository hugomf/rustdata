use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

pub fn expand_projection(input: DeriveInput) -> TokenStream {
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => panic!("Projection derive requires named fields"),
        },
        _ => panic!("Projection derive only supports structs"),
    };

    let field_defs: Vec<TokenStream> = fields
        .named
        .iter()
        .map(|f| {
            let col_name = f.ident.as_ref().unwrap().to_string();
            let sql_type = infer_sql_type(&f.ty);
            quote! {
                ::rustdata::column::ColumnDef::new(#col_name, #sql_type)
            }
        })
        .collect();

    let field_extracts: Vec<TokenStream> = fields
        .named
        .iter()
        .map(|f| {
            let field_ident = f.ident.as_ref().unwrap();
            let col_name = field_ident.to_string();
            let ty = &f.ty;
            quote! {
                #field_ident: <#ty as ::rustdata::sql_type::SqlExtract>
                    ::sql_extract(ext, row, #col_name)?,
            }
        })
        .collect();

    let result = quote! {
        impl ::rustdata::projection::Projection for #name {
            type Entity = #name;

            fn columns() -> &'static [::rustdata::column::ColumnDef] {
                &[
                    #(#field_defs),*
                ]
            }

            fn from_row<E: ::rustdata::descriptor::RowExtractor>(
                row: &E::Row,
                ext: &E,
            ) -> Result<#name, ::rustdata::error::RepositoryError> {
                use ::rustdata::sql_type::SqlExtract;
                Ok(#name {
                    #(#field_extracts)*
                })
            }
        }
    };
    result
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

fn infer_sql_type(ty: &Type) -> TokenStream {
    let inner = if is_option_type(ty) {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return infer_sql_type(inner);
                    }
                }
            }
        }
        return quote! { ::rustdata::column::SqlTypeId::Text };
    } else {
        ty
    };

    if let Type::Path(type_path) = inner {
        let last_seg = type_path.path.segments.last().map(|s| s.ident.to_string());
        match last_seg.as_deref() {
            Some("String") => quote! { ::rustdata::column::SqlTypeId::Varchar },
            Some("Uuid") => quote! { ::rustdata::column::SqlTypeId::Uuid },
            Some("DateTime") => quote! { ::rustdata::column::SqlTypeId::TimestampTz },
            Some("bool") | Some("Bool") => {
                quote! { ::rustdata::column::SqlTypeId::Boolean }
            }
            Some("i64") | Some("Int64") => {
                quote! { ::rustdata::column::SqlTypeId::BigInt }
            }
            Some("i32") | Some("Int32") => {
                quote! { ::rustdata::column::SqlTypeId::Int }
            }
            Some("f64") | Some("Float") | Some("f32") => {
                quote! { ::rustdata::column::SqlTypeId::Float }
            }
            Some("Vec") => quote! { ::rustdata::column::SqlTypeId::Jsonb },
            Some("HashSet") => quote! { ::rustdata::column::SqlTypeId::Jsonb },
            Some("Value") => quote! { ::rustdata::column::SqlTypeId::Jsonb },
            Some("serde_json") => quote! { ::rustdata::column::SqlTypeId::Jsonb },
            Some("NaiveDateTime") => {
                quote! { ::rustdata::column::SqlTypeId::TimestampTz }
            }
            _ => quote! { ::rustdata::column::SqlTypeId::Text },
        }
    } else {
        quote! { ::rustdata::column::SqlTypeId::Text }
    }
}

#[allow(dead_code)]
fn panic(msg: &str) -> ! {
    panic!("{}", msg);
}
