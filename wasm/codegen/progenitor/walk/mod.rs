//! Parse progenitor-generated code with `syn` into a `CodeModel`. Generic over Rust syntax,
//! except `ResponseValue<T>` (see `RustType::ResponseValue`). New shape-changing types need a
//! `RustType` variant; anything else falls through to `Named` and is handled in `emit/type_map.rs`.

mod extractions;
mod utils;

use std::collections::BTreeMap;
use std::path::Path;

use extractions::{extract_enum, extract_impl, extract_struct};
use syn::Item;
use utils::impl_target_name;

use super::{CodeModel, TypeInfo};

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

    let mut code_map = BTreeMap::new();

    for item in &file.items {
        item_walk(item, &[], &mut code_map)
    }

    CodeModel { items: code_map }
}

fn item_walk(item: &Item, module_path: &[String], code_map: &mut BTreeMap<String, TypeInfo>) {
    match item {
        // Skip trait impls.
        Item::Impl(imp) if imp.trait_.is_none() => {
            let target = impl_target_name(imp);
            if !target.is_empty() {
                let details = extract_impl(imp, &target, module_path);
                // Merge inherent impls (e.g. `impl EnumName { fn as_str() }`) into the existing
                // Struct/Enum entry instead of overwriting it.
                match code_map.get_mut(&target) {
                    Some(TypeInfo::Struct(s)) => s.methods.extend(details.methods),
                    Some(TypeInfo::Enum(e)) => e.methods.extend(details.methods),
                    Some(TypeInfo::Impl(i)) => i.methods.extend(details.methods),
                    None => {
                        code_map.insert(target, TypeInfo::Impl(details));
                    }
                }
            }
        }
        Item::Struct(s) => {
            if let Some(mut details) = extract_struct(s, module_path) {
                // Preserve methods from any impl block that was processed first.
                if let Some(TypeInfo::Impl(existing)) = code_map.remove(&details.name) {
                    details.methods.extend(existing.methods);
                }
                code_map.insert(details.name.clone(), TypeInfo::Struct(details));
            }
        }
        Item::Enum(e) => {
            if let Some(mut details) = extract_enum(e, module_path) {
                if let Some(TypeInfo::Impl(existing)) = code_map.remove(&details.name) {
                    details.methods.extend(existing.methods);
                }
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
