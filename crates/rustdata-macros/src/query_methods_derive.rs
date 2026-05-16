//! # `#[derive(QueryMethods)]` — typed find-by helpers
//!
//! Generates two local traits and their implementations so callers can write:
//!
//! ```ignore
//! // CrudRepository — table name comes from EntityDescriptor::TABLE
//! repo.find_by_age_gt(21).await?
//! repo.find_by_status_and_age("active", 30).await?
//!
//! // QueryRepository — table name supplied at call site
//! query.find_by_age_gt("users", 21).await?
//! query.find_by_status_and_age("users", "active", 30).await?
//! ```
//!
//! ## Why traits, not inherent impls
//!
//! Rust's coherence rule (E0116) forbids an inherent `impl` for a type defined
//! in another crate. `CrudRepository` and `QueryRepository` live in
//! `rustdata-core`, so writing `impl<BA> CrudRepository<BA, User> { … }` from
//! the user's crate (where this macro expands) is rejected.
//!
//! Fix: generate a *local* trait (belongs to the user's crate after expansion)
//! and implement it for the foreign repo types. The orphan rule allows
//! `impl LocalTrait for ForeignType` when the trait is local.
//!
//! Method bodies live in the `impl` block (not as trait defaults) so that
//! `self.find_all_pred(…)` resolves against the concrete inherent methods of
//! `CrudRepository` / `QueryRepository` rather than an abstract `Self`.
//!
//! ## Compound method ordering
//!
//! Both orderings are generated (e.g. `find_by_age_and_status` AND
//! `find_by_status_and_age`) so the call site can use whichever reads
//! naturally, regardless of struct field declaration order.

use darling::FromField;
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Ident};

#[derive(Debug, Default, FromField)]
#[darling(attributes(entity), default)]
struct QMFieldAttrs {
    id: bool,
    skip: bool,
    auto_generated: bool,
    column: Option<String>,
}

struct ColEntry {
    rust: String,
    col: String,
}

pub fn query_methods_derive(input: DeriveInput) -> proc_macro::TokenStream {
    let struct_name = &input.ident;

    let named_fields = match &input.data {
        syn::Data::Struct(ds) => match &ds.fields {
            syn::Fields::Named(nf) => &nf.named,
            _ => panic!("QueryMethods only supports structs with named fields"),
        },
        _ => panic!("QueryMethods only supports structs"),
    };

    let col_map: Vec<ColEntry> = named_fields
        .iter()
        .filter_map(|f| {
            let attrs = QMFieldAttrs::from_field(f).unwrap_or_default();
            if attrs.id || attrs.skip || attrs.auto_generated {
                return None;
            }
            let rust = f.ident.as_ref().unwrap().to_string();
            let col = attrs.column.unwrap_or_else(|| rust.clone());
            Some(ColEntry { rust, col })
        })
        .collect();

    // Separate vecs: trait signatures vs impl bodies
    let mut crud_sigs    = Vec::<proc_macro2::TokenStream>::new();
    let mut crud_methods = Vec::<proc_macro2::TokenStream>::new();
    let mut qrepo_sigs    = Vec::<proc_macro2::TokenStream>::new();
    let mut qrepo_methods = Vec::<proc_macro2::TokenStream>::new();

    // ── Single-field comparison methods ──────────────────────────────────
    for entry in &col_map {
        let col_name = &entry.col;

        let cmp_ops: &[(&str, &str)] = &[
            ("",     "Eq"),
            ("_ne",  "Ne"),
            ("_gt",  "Gt"),
            ("_gte", "Gte"),
            ("_lt",  "Lt"),
            ("_lte", "Lte"),
        ];

        for (suffix, variant_str) in cmp_ops {
            let variant  = Ident::new(variant_str, Span::call_site());
            let find_all = Ident::new(&format!("find_by_{}{}", entry.rust, suffix), Span::call_site());
            let find_one = Ident::new(&format!("find_one_by_{}{}", entry.rust, suffix), Span::call_site());

            crud_sigs.push(quote! {
                async fn #find_all<V>(&self, value: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;

                async fn #find_one<V>(&self, value: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;
            });

            crud_methods.push(quote! {
                async fn #find_all<V>(&self, value: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::#variant {
                        column: #col_name.to_string(),
                        value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                    };
                    self.find_all_pred(&predicate).await
                }

                async fn #find_one<V>(&self, value: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::#variant {
                        column: #col_name.to_string(),
                        value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                    };
                    self.find_one_pred(&predicate).await
                }
            });

            qrepo_sigs.push(quote! {
                async fn #find_all<V>(&self, table: &str, value: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;

                async fn #find_one<V>(&self, table: &str, value: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;
            });

            qrepo_methods.push(quote! {
                async fn #find_all<V>(&self, table: &str, value: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::#variant {
                        column: #col_name.to_string(),
                        value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                    };
                    self.find_all_pred(table, &predicate).await
                }

                async fn #find_one<V>(&self, table: &str, value: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::#variant {
                        column: #col_name.to_string(),
                        value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                    };
                    self.find_one_pred(table, &predicate).await
                }
            });
        }

        // ── LIKE ──
        {
            let find_all_like = Ident::new(&format!("find_by_{}_like", entry.rust), Span::call_site());
            let find_one_like = Ident::new(&format!("find_one_by_{}_like", entry.rust), Span::call_site());

            crud_sigs.push(quote! {
                async fn #find_all_like<V>(&self, pattern: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send;

                async fn #find_one_like<V>(&self, pattern: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send;
            });

            crud_methods.push(quote! {
                async fn #find_all_like<V>(&self, pattern: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Like {
                        column: #col_name.to_string(),
                        pattern: pattern.into(),
                    };
                    self.find_all_pred(&predicate).await
                }

                async fn #find_one_like<V>(&self, pattern: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Like {
                        column: #col_name.to_string(),
                        pattern: pattern.into(),
                    };
                    self.find_one_pred(&predicate).await
                }
            });

            qrepo_sigs.push(quote! {
                async fn #find_all_like<V>(&self, table: &str, pattern: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send;

                async fn #find_one_like<V>(&self, table: &str, pattern: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send;
            });

            qrepo_methods.push(quote! {
                async fn #find_all_like<V>(&self, table: &str, pattern: V)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Like {
                        column: #col_name.to_string(),
                        pattern: pattern.into(),
                    };
                    self.find_all_pred(table, &predicate).await
                }

                async fn #find_one_like<V>(&self, table: &str, pattern: V)
                    -> ::std::result::Result<::std::option::Option<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::std::convert::Into<::std::string::String> + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Like {
                        column: #col_name.to_string(),
                        pattern: pattern.into(),
                    };
                    self.find_one_pred(table, &predicate).await
                }
            });
        }

        // ── Paged single-field queries ──
        {
            for (suffix, variant_str) in &[("", "Eq"), ("_gt", "Gt"), ("_lt", "Lt"), ("_gte", "Gte"), ("_lte", "Lte")] {
                let variant   = Ident::new(variant_str, Span::call_site());
                let find_paged = Ident::new(&format!("find_by_{}{}_paged", entry.rust, suffix), Span::call_site());

                crud_sigs.push(quote! {
                    async fn #find_paged<V>(&self, value: V, pageable: &::rustdata_core::pagination::Pageable)
                        -> ::std::result::Result<::rustdata_core::pagination::Page<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send;
                });

                crud_methods.push(quote! {
                    async fn #find_paged<V>(&self, value: V, pageable: &::rustdata_core::pagination::Pageable)
                        -> ::std::result::Result<::rustdata_core::pagination::Page<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::#variant {
                            column: #col_name.to_string(),
                            value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                        };
                        self.find_all_pred_paged(&predicate, pageable).await
                    }
                });
            }
        }

        // ── IN ──
        {
            let find_in = Ident::new(&format!("find_by_{}_in", entry.rust), Span::call_site());

            crud_sigs.push(quote! {
                async fn #find_in<V>(&self, values: ::std::vec::Vec<V>)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;
            });

            crud_methods.push(quote! {
                async fn #find_in<V>(&self, values: ::std::vec::Vec<V>)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let sql_values = values.into_iter().map(::rustdata_core::specification::ToSqlValue::to_sql_value).collect();
                    let predicate = ::rustdata_core::specification::Predicate::In {
                        column: #col_name.to_string(),
                        values: sql_values,
                    };
                    self.find_all_pred(&predicate).await
                }
            });

            qrepo_sigs.push(quote! {
                async fn #find_in<V>(&self, table: &str, values: ::std::vec::Vec<V>)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send;
            });

            qrepo_methods.push(quote! {
                async fn #find_in<V>(&self, table: &str, values: ::std::vec::Vec<V>)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where V: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let sql_values = values.into_iter().map(::rustdata_core::specification::ToSqlValue::to_sql_value).collect();
                    let predicate = ::rustdata_core::specification::Predicate::In {
                        column: #col_name.to_string(),
                        values: sql_values,
                    };
                    self.find_all_pred(table, &predicate).await
                }
            });
        }

        // ── IS NULL / IS NOT NULL ──
        {
            let find_null     = Ident::new(&format!("find_by_{}_is_null",     entry.rust), Span::call_site());
            let find_not_null = Ident::new(&format!("find_by_{}_is_not_null", entry.rust), Span::call_site());
            let exists_null   = Ident::new(&format!("exists_by_{}_is_null",   entry.rust), Span::call_site());

            crud_sigs.push(quote! {
                async fn #find_null(&self)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>;
                async fn #find_not_null(&self)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>;
                async fn #exists_null(&self)
                    -> ::std::result::Result<bool, ::rustdata_core::error::RepositoryError>;
            });

            crud_methods.push(quote! {
                async fn #find_null(&self)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                {
                    let predicate = ::rustdata_core::specification::Predicate::IsNull { column: #col_name.to_string() };
                    self.find_all_pred(&predicate).await
                }

                async fn #find_not_null(&self)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                {
                    let predicate = ::rustdata_core::specification::Predicate::IsNotNull { column: #col_name.to_string() };
                    self.find_all_pred(&predicate).await
                }

                async fn #exists_null(&self)
                    -> ::std::result::Result<bool, ::rustdata_core::error::RepositoryError>
                {
                    let predicate = ::rustdata_core::specification::Predicate::IsNull { column: #col_name.to_string() };
                    self.find_all_pred(&predicate).await.map(|v| !v.is_empty())
                }
            });

            qrepo_sigs.push(quote! {
                async fn #find_null(&self, table: &str)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>;
                async fn #find_not_null(&self, table: &str)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>;
            });

            qrepo_methods.push(quote! {
                async fn #find_null(&self, table: &str)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                {
                    let predicate = ::rustdata_core::specification::Predicate::IsNull { column: #col_name.to_string() };
                    self.find_all_pred(table, &predicate).await
                }

                async fn #find_not_null(&self, table: &str)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                {
                    let predicate = ::rustdata_core::specification::Predicate::IsNotNull { column: #col_name.to_string() };
                    self.find_all_pred(table, &predicate).await
                }
            });
        }

        // ── count_by_* / exists_by_* / delete_by_* ──
        {
            for (suffix, variant_str) in &[("", "Eq"), ("_gt", "Gt"), ("_lt", "Lt"), ("_gte", "Gte"), ("_lte", "Lte")] {
                let variant   = Ident::new(variant_str, Span::call_site());
                let count_fn  = Ident::new(&format!("count_by_{}{}", entry.rust, suffix), Span::call_site());
                let exists_fn = Ident::new(&format!("exists_by_{}{}", entry.rust, suffix), Span::call_site());
                let delete_fn = Ident::new(&format!("delete_by_{}{}", entry.rust, suffix), Span::call_site());

                crud_sigs.push(quote! {
                    async fn #count_fn<V>(&self, value: V)
                        -> ::std::result::Result<u64, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send;

                    async fn #exists_fn<V>(&self, value: V)
                        -> ::std::result::Result<bool, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send;

                    async fn #delete_fn<V>(&self, value: V)
                        -> ::std::result::Result<u64, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send;
                });

                crud_methods.push(quote! {
                    async fn #count_fn<V>(&self, value: V)
                        -> ::std::result::Result<u64, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::#variant {
                            column: #col_name.to_string(),
                            value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                        };
                        self.count_pred(&predicate).await
                    }

                    async fn #exists_fn<V>(&self, value: V)
                        -> ::std::result::Result<bool, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::#variant {
                            column: #col_name.to_string(),
                            value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                        };
                        self.count_pred(&predicate).await.map(|c| c > 0)
                    }

                    async fn #delete_fn<V>(&self, value: V)
                        -> ::std::result::Result<u64, ::rustdata_core::error::RepositoryError>
                    where V: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::#variant {
                            column: #col_name.to_string(),
                            value:  ::rustdata_core::specification::ToSqlValue::to_sql_value(value),
                        };
                        self.delete_pred(&predicate).await
                    }
                });
            }
        }
    }

    // ── Compound AND with operator on second field ────────────────────────────
    // Generates find_by_status_and_age_gt, find_by_status_and_age_lt, etc.
    // First field always uses Eq; second field uses any comparison operator.
    for i in 0..col_map.len() {
        for j in 0..col_map.len() {
            if i == j { continue; }
            let ea = &col_map[i];
            let eb = &col_map[j];
            let ca = &ea.col;
            let cb = &eb.col;

            let fn_and = Ident::new(&format!("find_by_{}_and_{}", ea.rust, eb.rust), Span::call_site());
            let fn_or  = Ident::new(&format!("find_by_{}_or_{}",  ea.rust, eb.rust), Span::call_site());

            crud_sigs.push(quote! {
                async fn #fn_and<VA, VB>(&self, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send;

                async fn #fn_or<VA, VB>(&self, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send;
            });

            crud_methods.push(quote! {
                async fn #fn_and<VA, VB>(&self, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::And(vec![
                        ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                        ::rustdata_core::specification::Predicate::Eq { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                    ]);
                    self.find_all_pred(&predicate).await
                }

                async fn #fn_or<VA, VB>(&self, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Or(vec![
                        ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                        ::rustdata_core::specification::Predicate::Eq { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                    ]);
                    self.find_all_pred(&predicate).await
                }
            });

            qrepo_sigs.push(quote! {
                async fn #fn_and<VA, VB>(&self, table: &str, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send;

                async fn #fn_or<VA, VB>(&self, table: &str, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send;
            });

            qrepo_methods.push(quote! {
                async fn #fn_and<VA, VB>(&self, table: &str, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::And(vec![
                        ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                        ::rustdata_core::specification::Predicate::Eq { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                    ]);
                    self.find_all_pred(table, &predicate).await
                }

                async fn #fn_or<VA, VB>(&self, table: &str, va: VA, vb: VB)
                    -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                where VA: ::rustdata_core::specification::ToSqlValue + Send,
                      VB: ::rustdata_core::specification::ToSqlValue + Send,
                {
                    let predicate = ::rustdata_core::specification::Predicate::Or(vec![
                        ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                        ::rustdata_core::specification::Predicate::Eq { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                    ]);
                    self.find_all_pred(table, &predicate).await
                }
            });

            // Compound with comparison operators on the second field:
            // find_by_{a}_and_{b}_gt, find_by_{a}_and_{b}_lt, etc.
            let richer_ops: &[(&str, &str)] = &[
                ("_gt",  "Gt"),
                ("_gte", "Gte"),
                ("_lt",  "Lt"),
                ("_lte", "Lte"),
                ("_ne",  "Ne"),
            ];
            for (op_suffix, op_variant_str) in richer_ops {
                let op_variant = Ident::new(op_variant_str, Span::call_site());
                let fn_richer = Ident::new(
                    &format!("find_by_{}_and_{}{}", ea.rust, eb.rust, op_suffix),
                    Span::call_site(),
                );

                crud_sigs.push(quote! {
                    async fn #fn_richer<VA, VB>(&self, va: VA, vb: VB)
                        -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where VA: ::rustdata_core::specification::ToSqlValue + Send,
                          VB: ::rustdata_core::specification::ToSqlValue + Send;
                });

                crud_methods.push(quote! {
                    async fn #fn_richer<VA, VB>(&self, va: VA, vb: VB)
                        -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where VA: ::rustdata_core::specification::ToSqlValue + Send,
                          VB: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::And(vec![
                            ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                            ::rustdata_core::specification::Predicate::#op_variant { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                        ]);
                        self.find_all_pred(&predicate).await
                    }
                });

                qrepo_sigs.push(quote! {
                    async fn #fn_richer<VA, VB>(&self, table: &str, va: VA, vb: VB)
                        -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where VA: ::rustdata_core::specification::ToSqlValue + Send,
                          VB: ::rustdata_core::specification::ToSqlValue + Send;
                });

                qrepo_methods.push(quote! {
                    async fn #fn_richer<VA, VB>(&self, table: &str, va: VA, vb: VB)
                        -> ::std::result::Result<::std::vec::Vec<#struct_name>, ::rustdata_core::error::RepositoryError>
                    where VA: ::rustdata_core::specification::ToSqlValue + Send,
                          VB: ::rustdata_core::specification::ToSqlValue + Send,
                    {
                        let predicate = ::rustdata_core::specification::Predicate::And(vec![
                            ::rustdata_core::specification::Predicate::Eq { column: #ca.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(va) },
                            ::rustdata_core::specification::Predicate::#op_variant { column: #cb.to_string(), value: ::rustdata_core::specification::ToSqlValue::to_sql_value(vb) },
                        ]);
                        self.find_all_pred(table, &predicate).await
                    }
                });
            }
        }
    }

    // ── Where-clause bounds ────────────────────────────────────────────────
    let crud_bounds = quote! {
        BA: ::rustdata_core::backend::DbBound,
        for<'q> <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Arguments<'q>:
            ::sqlx::IntoArguments<'q, ::rustdata_core::backend::DbOf<BA>>,
        for<'c> &'c mut <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Connection:
            ::sqlx::Executor<'c, Database = ::rustdata_core::backend::DbOf<BA>>,
        ::rustdata_core::backend::ExOf<BA>: ::rustdata_core::descriptor::RowExtractor<
            Row = <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Row,
        >,
        #struct_name: ::rustdata_core::entity::EntityDescriptor
            + ::rustdata_core::lifecycle::LifecycleHooks<
                <#struct_name as ::rustdata_core::entity::EntityDescriptor>::Entity,
            >,
        <#struct_name as ::rustdata_core::entity::EntityDescriptor>::Id: Clone,
    };

    let qrepo_bounds = quote! {
        BA: ::rustdata_core::backend::DbBound,
        for<'q> <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Arguments<'q>:
            ::sqlx::IntoArguments<'q, ::rustdata_core::backend::DbOf<BA>>,
        for<'c> &'c mut <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Connection:
            ::sqlx::Executor<'c, Database = ::rustdata_core::backend::DbOf<BA>>,
        ::rustdata_core::backend::ExOf<BA>: ::rustdata_core::descriptor::RowExtractor<
            Row = <::rustdata_core::backend::DbOf<BA> as ::sqlx::Database>::Row,
        >,
        #struct_name: ::rustdata_core::row_extractable::RowExtractable,
    };

    let crud_trait_name  = Ident::new(&format!("{}CrudQueryMethods",  struct_name), Span::call_site());
    let qrepo_trait_name = Ident::new(&format!("{}QueryQueryMethods", struct_name), Span::call_site());

    let out = quote! {
        // Trait declares signatures; impl provides bodies on the concrete type.
        // Self in the impl = CrudRepository<BA, #struct_name>, so
        // self.find_all_pred / find_one_pred resolve as inherent methods.
        #[allow(non_camel_case_types, dead_code, async_fn_in_trait)]
        pub trait #crud_trait_name<BA>
        where #crud_bounds
        {
            #(#crud_sigs)*
        }

        #[allow(dead_code, async_fn_in_trait)]
        impl<BA> #crud_trait_name<BA> for ::rustdata_core::repo::CrudRepository<BA, #struct_name>
        where #crud_bounds
        {
            #(#crud_methods)*
        }

        #[allow(non_camel_case_types, dead_code, async_fn_in_trait)]
        pub trait #qrepo_trait_name<BA>
        where #qrepo_bounds
        {
            #(#qrepo_sigs)*
        }

        #[allow(dead_code, async_fn_in_trait)]
        impl<BA> #qrepo_trait_name<BA> for ::rustdata_core::row_extractable::QueryRepository<BA, #struct_name>
        where #qrepo_bounds
        {
            #(#qrepo_methods)*
        }
    };

    proc_macro::TokenStream::from(out)
}
