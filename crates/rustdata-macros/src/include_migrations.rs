//! `include_migrations!(path)` — compile-time glob of migration SQL files.
//!
//! Emits a `&'static [(&'static str, &'static str)]` literal where each entry
//! is `(file_stem, sql_text)`, sorted by numeric version prefix.
//!
//! This proc-macro is called exclusively from `rustdata_migrations::migrate!`.
//! Developers never invoke it directly.

use proc_macro2::TokenStream;
use quote::quote;
use std::path::PathBuf;

pub fn expand_include_migrations(path_lit: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the string literal argument, e.g. "migrations"
    let path_str: String = {
        let s = path_lit.to_string();
        // Strip surrounding quotes if present
        s.trim().trim_matches('"').to_string()
    };

    // Resolve relative to the crate that invoked the macro (CARGO_MANIFEST_DIR)
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo");

    let migrations_dir = PathBuf::from(&manifest_dir).join(&path_str);

    // Collect and sort .sql files
    let mut entries: Vec<(u64, String, PathBuf)> = std::fs::read_dir(&migrations_dir)
        .unwrap_or_else(|e| {
            panic!(
                "include_migrations!: cannot read directory `{}`: {}",
                migrations_dir.display(),
                e
            )
        })
        .filter_map(|res| res.ok())
        .filter(|e| e.path().extension().map(|x| x == "sql").unwrap_or(false))
        .filter_map(|entry| {
            let path = entry.path();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())?;
            let version = parse_version(&stem)?;
            Some((version, stem, path))
        })
        .collect();

    entries.sort_by_key(|(v, _, _)| *v);

    // Build the literal pairs
    let pairs: Vec<TokenStream> = entries
        .iter()
        .map(|(_, stem, path)| {
            let path_str = path.to_str().expect("non-UTF8 path");
            quote! {
                (#stem, include_str!(#path_str))
            }
        })
        .collect();

    quote! {
        &[ #(#pairs),* ]
    }
    .into()
}

/// Parse the leading numeric version from a migration file stem.
/// Strips a leading `v`, `V`, `m`, or `M` then reads until the first `_` or `-`.
fn parse_version(stem: &str) -> Option<u64> {
    let s = stem
        .strip_prefix('v')
        .or_else(|| stem.strip_prefix('V'))
        .or_else(|| stem.strip_prefix('m'))
        .or_else(|| stem.strip_prefix('M'))
        .unwrap_or(stem);

    let prefix = s
        .split(|c: char| ['_', '-'].contains(&c))
        .next()
        .unwrap_or(s);

    prefix.parse::<u64>().ok()
}
