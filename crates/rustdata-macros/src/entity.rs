use darling::{FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

#[derive(Debug, Default, FromDeriveInput)]
#[darling(attributes(entity), default)]
struct EntityAttrs {
    table: Option<String>,
    order_by: Option<String>,
    soft_delete: Option<String>,
    entity_type: Option<String>,
    /// Path to a type that implements `LifecycleHooks`.
    /// When set, the derive skips emitting a blank `LifecycleHooks` impl
    /// so the custom one can be provided instead.
    /// Example: `#[entity(hooks = "MyEntityHooks")]`
    hooks: Option<String>,
}

#[derive(Debug, Default, FromField)]
#[darling(attributes(entity), default)]
struct FieldAttrs {
    id: bool,
    skip: bool,
    json: bool,
    insert_only: bool,
    auto_generated: bool,
    version: bool,
    column: Option<String>,
    map: Option<String>,
}

struct FieldInfo {
    ident: syn::Ident,
    ty: syn::Type,
    attrs: FieldAttrs,
    col_name: String,
}

pub fn expand_derive(input: DeriveInput) -> TokenStream {
    let entity_attrs =
        EntityAttrs::from_derive_input(&input).expect("failed to parse #[entity(...)] attributes");
    let table = entity_attrs
        .table
        .expect("#[entity(table = \"...\")] is required");
    let order_by = entity_attrs
        .order_by
        .unwrap_or_else(|| "id ASC".to_string());
    let soft_delete = entity_attrs.soft_delete.as_deref();
    let soft_delete_col = soft_delete
        .map(|s| quote! { Some(#s) })
        .unwrap_or(quote! { None });

    let struct_name = &input.ident;
    let entity_type: Option<syn::Type> = entity_attrs
        .entity_type
        .as_ref()
        .map(|s| syn::parse_str(s).expect("entity_type must be a valid Rust type path"));
    let entity_ctor = match &entity_type {
        Some(et) => quote! { #et },
        None => quote! { #struct_name },
    };

    // If the user supplies #[entity(hooks = "MyHooks")], the trait impl is
    // delegated to that type and we skip the blank default impl.  Otherwise
    // we emit `impl LifecycleHooks<Entity> for Struct {}` so that the trait
    // bound on CrudRepository is satisfied without any user boilerplate.
    let hooks_impl = match entity_attrs.hooks.as_deref() {
        Some(hooks_path) => {
            let hooks_ty: syn::Type = syn::parse_str(hooks_path)
                .expect("hooks attribute must be a valid Rust type path");
            quote! {
                impl ::rustdata_core::lifecycle::LifecycleHooks<#entity_ctor> for #struct_name {
                    fn before_save(entity: &mut #entity_ctor) -> ::std::result::Result<(), ::rustdata_core::error::RepositoryError> {
                        <#hooks_ty as ::rustdata_core::lifecycle::LifecycleHooks<#entity_ctor>>::before_save(entity)
                    }
                    fn after_save(entity: &#entity_ctor) -> ::std::result::Result<(), ::rustdata_core::error::RepositoryError> {
                        <#hooks_ty as ::rustdata_core::lifecycle::LifecycleHooks<#entity_ctor>>::after_save(entity)
                    }
                }
            }
        }
        None => quote! {
            impl ::rustdata_core::lifecycle::LifecycleHooks<#entity_ctor> for #struct_name {}
        },
    };

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => panic("Entity derive requires named fields"),
        },
        _ => panic("Entity derive only supports structs"),
    };

    let field_infos: Vec<FieldInfo> = fields
        .named
        .iter()
        .map(|f| {
            let attrs = FieldAttrs::from_field(f).unwrap_or_default();
            let col_name = attrs
                .column
                .clone()
                .unwrap_or_else(|| f.ident.as_ref().unwrap().to_string());
            FieldInfo {
                ident: f.ident.clone().unwrap(),
                ty: f.ty.clone(),
                attrs,
                col_name,
            }
        })
        .collect();

    let id_field = field_infos
        .iter()
        .find(|f| f.attrs.id)
        .expect("One field must have #[entity(id)]");
    let id_type = &id_field.ty;
    let _id_col = &id_field.col_name;

    let non_skip_fields: Vec<&FieldInfo> = field_infos.iter().filter(|f| !f.attrs.skip).collect();

    let column_defs: Vec<TokenStream> = non_skip_fields
        .iter()
        .map(|f| build_column_def(f))
        .collect();

    let insert_fields: Vec<&FieldInfo> = field_infos
        .iter()
        .filter(|f| !f.attrs.skip && !f.attrs.auto_generated)
        .collect();

    let bind_insert_calls: Vec<TokenStream> = insert_fields
        .iter()
        .map(|f| build_bind_call(f, true))
        .collect();

    let update_fields: Vec<&FieldInfo> = field_infos
        .iter()
        .filter(|f| !f.attrs.skip && !f.attrs.id && !f.attrs.insert_only && !f.attrs.auto_generated)
        .collect();

    let mut bind_update_calls: Vec<TokenStream> = update_fields
        .iter()
        .map(|f| build_bind_call(f, true))
        .collect();

    // ID field bound at end for WHERE clause (matches update_sql which
    // places WHERE id = $N after SET columns)
    let bind_update_id_call = build_bind_call(id_field, true);
    bind_update_calls.push(bind_update_id_call);

    let from_row_fields: Vec<TokenStream> = field_infos
        .iter()
        .map(|f| {
            if f.attrs.skip {
                let field_ident = &f.ident;
                quote! { #field_ident: ::core::default::Default::default(), }
            } else {
                build_extract_call(f)
            }
        })
        .collect();

    let repo_name = syn::Ident::new(&format!("{}Repo", struct_name), struct_name.span());

    let bind_id_call = if id_field.attrs.json {
        quote! {
            let query = {
                let _json = ::serde_json::to_value(id).unwrap_or_default();
                B::bind_json_value(query, _json)
            };
            query
        }
    } else {
        quote! {
            <#id_type as ::rustdata_core::sql_type::SqlBind>
                ::sql_bind::<DB, B>(query, id)
        }
    };

    let soft_delete_val = soft_delete_col;

    let result = quote! {
        #hooks_impl

        impl ::rustdata_core::descriptor::EntityDescriptor for #struct_name {
            type Entity = #entity_ctor;
            type Id = #id_type;

            const TABLE: &'static str = #table;
            const ORDER_BY: &'static str = #order_by;
            const SOFT_DELETE_COL: Option<&'static str> = #soft_delete_val;

            fn columns() -> &'static [::rustdata_core::column::ColumnDef] {
                const COLS: &[::rustdata_core::column::ColumnDef] = &[
                    #(#column_defs),*
                ];
                COLS
            }

            fn bind_insert<'q, DB, B>(
                query: ::rustdata_core::bind::QueryBuilder<'q, DB>,
                entity: &'q Self::Entity,
            ) -> ::rustdata_core::bind::QueryBuilder<'q, DB>
            where
                DB: sqlx::Database,
                B: ::rustdata_core::bind::BindAdapter<DB>,
            {
                use ::rustdata_core::sql_type::SqlBind;
                #(#bind_insert_calls)*
                query
            }

            fn bind_update<'q, DB, B>(
                query: ::rustdata_core::bind::QueryBuilder<'q, DB>,
                entity: &'q Self::Entity,
            ) -> ::rustdata_core::bind::QueryBuilder<'q, DB>
            where
                DB: sqlx::Database,
                B: ::rustdata_core::bind::BindAdapter<DB>,
            {
                use ::rustdata_core::sql_type::SqlBind;
                #(#bind_update_calls)*
                query
            }

            fn bind_id<'q, DB, B>(
                query: ::rustdata_core::bind::QueryBuilder<'q, DB>,
                id: &'q Self::Id,
            ) -> ::rustdata_core::bind::QueryBuilder<'q, DB>
            where
                DB: sqlx::Database,
                B: ::rustdata_core::bind::BindAdapter<DB>,
            {
                use ::rustdata_core::sql_type::SqlBind;
                #bind_id_call
            }

            fn from_row<E: ::rustdata_core::descriptor::RowExtractor>(
                row: &E::Row,
                ext: &E,
            ) -> Result<Self::Entity, ::rustdata_core::error::RepositoryError> {
                use ::rustdata_core::sql_type::SqlExtract;
                Ok(#entity_ctor {
                    #(#from_row_fields)*
                })
            }
        }

        // --- RowExtractable: lets the entity struct be used with QueryRepository ---
        impl ::rustdata_core::RowExtractable for #struct_name {
            fn extract_row<E: ::rustdata_core::descriptor::RowExtractor>(
                row: &E::Row,
                extractor: &E,
            ) -> Result<Self, ::rustdata_core::error::RepositoryError> {
                <#struct_name as ::rustdata_core::descriptor::EntityDescriptor>::from_row(row, extractor)
            }
        }
        // --- Concrete repo type alias — pinned to the active backend ---
        //
        // `DefaultBackend` is a type alias defined in `rustdata-core` and
        // resolved there using its own feature flags (sqlite/postgres/mysql).
        // We reference it here so the proc-macro crate never needs backend
        // features of its own — the feature gate lives where it belongs.
        pub type #repo_name = ::rustdata_core::repo::CrudRepository<
            ::rustdata_core::DefaultBackend,
            #struct_name,
        >;
    };
    result
}

fn build_column_def(f: &FieldInfo) -> TokenStream {
    let col_name = &f.col_name;
    let sql_type = infer_sql_type(&f.ty);

    let mut builder = quote! {
        ::rustdata_core::column::ColumnDef::new(#col_name, #sql_type)
    };

    if f.attrs.id {
        builder = quote! { #builder.id() };
    }
    if is_option_type(&f.ty) {
        builder = quote! { #builder.nullable() };
    }
    if f.attrs.json {
        builder = quote! { #builder.json() };
    }
    if f.attrs.auto_generated {
        builder = quote! {
            #builder
                .insert(::rustdata_core::column::InsertStrategy::AutoGenerated)
                .update(::rustdata_core::column::UpdateStrategy::Immutable)
        };
    } else if f.attrs.insert_only {
        builder = quote! {
            #builder
                .insert(::rustdata_core::column::InsertStrategy::Provided)
                .update(::rustdata_core::column::UpdateStrategy::Immutable)
        };
    } else if f.attrs.version {
        builder = quote! {
            #builder.update(::rustdata_core::column::UpdateStrategy::Conditional)
        };
    }

    builder
}

fn build_bind_call(f: &FieldInfo, _with_semicolon: bool) -> TokenStream {
    let field_ident = &f.ident;
    if f.attrs.json {
        quote! {
            let query = {
                let _json = ::serde_json::to_value(&entity.#field_ident).unwrap_or_default();
                B::bind_json_value(query, _json)
            };
        }
    } else {
        let ty = &f.ty;
        quote! {
            let query = <#ty as ::rustdata_core::sql_type::SqlBind>
                ::sql_bind::<DB, B>(query, &entity.#field_ident);
        }
    }
}

fn build_extract_call(f: &FieldInfo) -> TokenStream {
    let field_ident = &f.ident;
    let col_name = &f.col_name;
    let extract_expr = if f.attrs.json {
        let ty = &f.ty;
        quote! { ext.get_json::<#ty>(row, #col_name)? }
    } else {
        let ty = &f.ty;
        quote! { <#ty as ::rustdata_core::sql_type::SqlExtract>::sql_extract(ext, row, #col_name)? }
    };
    if let Some(map_str) = &f.attrs.map {
        let map_expr: syn::Expr = syn::parse_str(map_str).expect(
            "map attribute value must be a valid Rust expression (closure like |v| v.to_rfc3339())",
        );
        quote! {
            #field_ident: {
                let _v = #extract_expr;
                (#map_expr)(_v)
            },
        }
    } else {
        quote! {
            #field_ident: #extract_expr,
        }
    }
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
        return quote! { ::rustdata_core::column::SqlTypeId::Text };
    } else {
        ty
    };

    if let Type::Path(type_path) = inner {
        let last_seg = type_path.path.segments.last().map(|s| s.ident.to_string());
        match last_seg.as_deref() {
            Some("String") => quote! { ::rustdata_core::column::SqlTypeId::Varchar },
            Some("Uuid") => quote! { ::rustdata_core::column::SqlTypeId::Uuid },
            Some("DateTime") => quote! { ::rustdata_core::column::SqlTypeId::TimestampTz },
            Some("bool") | Some("Bool") => {
                quote! { ::rustdata_core::column::SqlTypeId::Boolean }
            }
            Some("i64") | Some("Int64") => {
                quote! { ::rustdata_core::column::SqlTypeId::BigInt }
            }
            Some("i32") | Some("Int32") => {
                quote! { ::rustdata_core::column::SqlTypeId::Int }
            }
            Some("f64") | Some("Float") | Some("f32") => {
                quote! { ::rustdata_core::column::SqlTypeId::Float }
            }
            Some("Vec") => quote! { ::rustdata_core::column::SqlTypeId::Jsonb },
            Some("HashSet") => quote! { ::rustdata_core::column::SqlTypeId::Jsonb },
            Some("Value") => quote! { ::rustdata_core::column::SqlTypeId::Jsonb },
            Some("serde_json") => quote! { ::rustdata_core::column::SqlTypeId::Jsonb },
            Some("NaiveDateTime") => {
                quote! { ::rustdata_core::column::SqlTypeId::TimestampTz }
            }
            _ => quote! { ::rustdata_core::column::SqlTypeId::Text },
        }
    } else {
        quote! { ::rustdata_core::column::SqlTypeId::Text }
    }
}

fn panic(msg: &str) -> ! {
    panic!("{}", msg);
}