//! Parse progenitor-generated Rust code with `syn` and build a `CodeModel`.
//!
//! This module is intentionally generic — it knows about Rust syntax, not WASM
//! or progenitor semantics. The one exception is `ResponseValue<T>`: we promote
//! it to a `RustType` variant because it appears on every client method return.
//! See the doc comment on `RustType::ResponseValue` for rationale.
//!
//! # Adding new type support
//!
//! 1. If it's a structural type that changes codegen shape (like `Option`, `Vec`),
//!    add a variant to `RustType` and handle it in `utils::parse_rust_type`.
//! 2. Otherwise, it falls through to `Named { name, args }` automatically.
//!    Handle it in `emit/type_map.rs` by matching on the name.

mod extractions;
mod utils;

use std::collections::HashMap;
use std::path::Path;

use syn::Item;

use super::{CodeModel, TypeInfo};
use extractions::{extract_enum, extract_impl, extract_struct};
use utils::impl_target_name;

// ── Entry Point ──────────────────────────────────────────────────────────────

/// Parse the progenitor-generated codegen.rs and build a `CodeModel`.
pub fn extract_code_model(codegen_path: &Path) -> CodeModel {
    let source = std::fs::read_to_string(codegen_path).unwrap_or_else(|e| {
        panic!(
            "failed to read progenitor codegen at {}: {e}",
            codegen_path.display()
        )
    });

    let file = syn::parse_file(&source)
        .unwrap_or_else(|e| panic!("failed to parse progenitor codegen: {e}"));

    let mut code_map = HashMap::new();

    for item in &file.items {
        item_walk(item, &[], &mut code_map)
    }

    CodeModel { items: code_map }
}

fn item_walk(item: &Item, module_path: &[String], code_map: &mut HashMap<String, TypeInfo>) {
    match item {
        // We dont want to reimplement traits.
        Item::Impl(imp) if imp.trait_.is_none() => {
            let target = impl_target_name(imp);
            if !target.is_empty() {
                let details = extract_impl(imp, &target, module_path);
                code_map.insert(target, TypeInfo::Impl(details));
            }
        }
        Item::Struct(s) => {
            if let Some(details) = extract_struct(s, module_path) {
                code_map.insert(details.name.clone(), TypeInfo::Struct(details));
            }
        }
        Item::Enum(e) => {
            if let Some(details) = extract_enum(e, module_path) {
                code_map.insert(details.name.clone(), TypeInfo::Enum(details));
            }
        }
        Item::Mod(module) => {
            if let Some((_, inner_items)) = &module.content {
                let mut child_path = module_path.to_vec();
                child_path.push(module.ident.to_string());
                for inner in inner_items {
                    item_walk(inner, &child_path, code_map);
                }
            }
        }
        _ => {}
    }
}
