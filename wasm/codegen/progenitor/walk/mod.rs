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
//!    add a variant to `RustType` and handle it in `parse_rust_type`.
//! 2. Otherwise, it falls through to `Named { name, args }` automatically.
//!    Handle it in `emit/type_map.rs` by matching on the name.

use std::collections::HashMap;
use std::path::Path;

use syn::{
    FnArg, GenericArgument, ImplItem, Item, ItemEnum, ItemImpl, ItemStruct, Pat, PathArguments,
    ReturnType, Type,
};

use super::{
    CodeModel, EnumDetails, FieldDetails, ImplDetails, MethodDetails, ParamDetails, Primitive,
    RustType, StructDetails, TypeInfo, VariantDetails,
};

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

// ── Struct Extraction ────────────────────────────────────────────────────────

fn extract_struct(s: &ItemStruct, module_path: &[String]) -> Option<StructDetails> {
    let name = s.ident.to_string();

    // Check for #[serde(transparent)] — marks newtype wrappers.
    let is_newtype = has_serde_transparent(&s.attrs);
    let derives = extract_derives(&s.attrs);

    match &s.fields {
        syn::Fields::Named(named) => {
            let fields = named
                .named
                .iter()
                .filter_map(|f| {
                    let field_name = f.ident.as_ref()?.to_string();
                    let ty = parse_rust_type(&f.ty)?;
                    let serde_rename = extract_serde_rename(&f.attrs);
                    Some(FieldDetails {
                        name: field_name,
                        ty,
                        serde_rename,
                    })
                })
                .collect();

            Some(StructDetails {
                name,
                fields,
                is_newtype,
                module_path: module_path.to_vec(),
                derives,
            })
        }
        syn::Fields::Unnamed(_) => {
            // Tuple struct — treat as newtype with no exposed fields.
            Some(StructDetails {
                name,
                fields: vec![],
                is_newtype: true,
                module_path: module_path.to_vec(),
                derives,
            })
        }
        syn::Fields::Unit => None,
    }
}

fn has_serde_transparent(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("serde") {
            return false;
        }
        let Ok(nested) = attr.parse_args::<syn::Meta>() else {
            return false;
        };
        matches!(&nested, syn::Meta::Path(p) if p.is_ident("transparent"))
    })
}

/// Extract derive macro names from attributes.
///
/// Parses `#[derive(Serialize, Deserialize, Clone)]` → `["Serialize", "Deserialize", "Clone"]`.
/// Also handles paths like `serde::Serialize` or `::serde::Serialize` → `["serde::Serialize"]`.
fn extract_derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut derives = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        if let Ok(meta) = attr.meta.require_list() {
            meta.parse_nested_meta(|nested| {
                // Collect the full path (e.g., `serde::Serialize` or just `Clone`)
                let path_str = nested
                    .path
                    .segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                derives.push(path_str);
                Ok(())
            })
            .ok();
        }
    }
    derives
}

fn extract_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        // Parse #[serde(rename = "camelCase")]
        let Ok(nested) = attr.parse_args::<syn::MetaNameValue>() else {
            continue;
        };
        if nested.path.is_ident("rename") {
            if let syn::Expr::Lit(lit) = &nested.value {
                if let syn::Lit::Str(s) = &lit.lit {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

// ── Enum Extraction ──────────────────────────────────────────────────────────

fn extract_enum(e: &ItemEnum, module_path: &[String]) -> Option<EnumDetails> {
    let name = e.ident.to_string();
    let derives = extract_derives(&e.attrs);

    let variants = e
        .variants
        .iter()
        .map(|v| {
            let variant_name = v.ident.to_string();
            let fields = match &v.fields {
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .filter_map(|f| {
                        let field_name = f.ident.as_ref()?.to_string();
                        let ty = parse_rust_type(&f.ty)?;
                        let serde_rename = extract_serde_rename(&f.attrs);
                        Some(FieldDetails {
                            name: field_name,
                            ty,
                            serde_rename,
                        })
                    })
                    .collect(),
                syn::Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        let ty = parse_rust_type(&f.ty)?;
                        Some(FieldDetails {
                            name: format!("_{i}"),
                            ty,
                            serde_rename: None,
                        })
                    })
                    .collect(),
                syn::Fields::Unit => vec![],
            };
            VariantDetails {
                name: variant_name,
                fields,
            }
        })
        .collect();

    Some(EnumDetails {
        name,
        variants,
        module_path: module_path.to_vec(),
        derives,
    })
}

// ── Impl Extraction ──────────────────────────────────────────────────────────

fn extract_impl(imp: &ItemImpl, target: &str, module_path: &[String]) -> ImplDetails {
    let methods = imp
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(method) = item {
                extract_method(method)
            } else {
                None
            }
        })
        .collect();

    ImplDetails {
        target: target.to_string(),
        methods,
        module_path: module_path.to_vec(),
    }
}

fn extract_method(method: &syn::ImplItemFn) -> Option<MethodDetails> {
    let sig = &method.sig;
    let name = sig.ident.to_string();
    let is_async = sig.asyncness.is_some();

    let params = sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_ty) = arg {
                let param_name = match pat_ty.pat.as_ref() {
                    Pat::Ident(id) => id.ident.to_string(),
                    _ => return None,
                };
                let ty = parse_rust_type(&pat_ty.ty)?;
                Some(ParamDetails {
                    name: param_name,
                    ty,
                })
            } else {
                None // Skip &self
            }
        })
        .collect();

    let return_type = parse_return_type(&sig.output);

    Some(MethodDetails {
        name,
        is_async,
        params,
        return_type,
    })
}

// ── Type Parsing ─────────────────────────────────────────────────────────────

/// Parse any `syn::Type` into our `RustType` IR.
///
/// This is the core generic parser. It handles:
/// - Primitives (bool, i32, String, etc.)
/// - Structural types (Option, Vec, Map, tuples)
/// - References (&T, &[T])
/// - Named types with generics (falls through to `Named`)
fn parse_rust_type(ty: &Type) -> Option<RustType> {
    match ty {
        Type::Path(tp) => parse_type_path(tp),
        Type::Reference(r) => parse_reference(r),
        Type::Tuple(t) => {
            let inner: Vec<RustType> = t.elems.iter().filter_map(parse_rust_type).collect();
            Some(RustType::Tuple(inner))
        }
        Type::Slice(s) => {
            let inner = parse_rust_type(&s.elem)?;
            Some(RustType::Slice(Box::new(inner)))
        }
        _ => None,
    }
}

fn parse_type_path(tp: &syn::TypePath) -> Option<RustType> {
    let seg = tp.path.segments.last()?;
    let ident = seg.ident.to_string();

    // Check for fully-qualified paths first
    if tp.path.segments.len() > 1 {
        let full = path_to_string(&tp.path);
        if full.ends_with("::String") || full == "std::string::String" {
            return Some(RustType::String);
        }
    }

    match ident.as_str() {
        // Primitives
        "bool" => Some(RustType::Bool),
        "String" => Some(RustType::String),
        "str" => Some(RustType::String), // &str reference will wrap this
        "i8" => Some(RustType::Primitive(Primitive::I8)),
        "i16" => Some(RustType::Primitive(Primitive::I16)),
        "i32" => Some(RustType::Primitive(Primitive::I32)),
        "i64" => Some(RustType::Primitive(Primitive::I64)),
        "u8" => Some(RustType::Primitive(Primitive::U8)),
        "u16" => Some(RustType::Primitive(Primitive::U16)),
        "u32" => Some(RustType::Primitive(Primitive::U32)),
        "u64" => Some(RustType::Primitive(Primitive::U64)),
        "f32" => Some(RustType::Primitive(Primitive::F32)),
        "f64" => Some(RustType::Primitive(Primitive::F64)),

        // Structural types
        "Option" => {
            let inner = first_generic_arg(&seg.arguments)?;
            let inner_ty = parse_rust_type(inner)?;
            Some(RustType::Option(Box::new(inner_ty)))
        }
        "Vec" => {
            let inner = first_generic_arg(&seg.arguments)?;
            let inner_ty = parse_rust_type(inner)?;
            Some(RustType::Vec(Box::new(inner_ty)))
        }
        "Map" | "HashMap" => {
            let (k, v) = first_two_generic_args(&seg.arguments)?;
            let k_ty = parse_rust_type(k)?;
            let v_ty = parse_rust_type(v)?;
            Some(RustType::Map(Box::new(k_ty), Box::new(v_ty)))
        }

        // Everything else: Named with optional generic args
        _ => {
            let args = collect_generic_args(&seg.arguments);
            Some(RustType::Named { name: ident, args })
        }
    }
}

fn parse_reference(r: &syn::TypeReference) -> Option<RustType> {
    // &[T] — reference to slice
    if let Type::Slice(s) = r.elem.as_ref() {
        let inner = parse_rust_type(&s.elem)?;
        return Some(RustType::Ref(Box::new(RustType::Slice(Box::new(inner)))));
    }

    // &T
    let inner = parse_rust_type(&r.elem)?;
    Some(RustType::Ref(Box::new(inner)))
}

/// Parse a method return type, unwrapping `Result<ResponseValue<T>, Error<_>>`.
///
/// This is the one progenitor-aware function in walk. We promote `ResponseValue`
/// because it appears on every client method return. See `RustType::ResponseValue` docs.
fn parse_return_type(ret: &ReturnType) -> Option<RustType> {
    let ty = match ret {
        ReturnType::Type(_, ty) => ty.as_ref(),
        ReturnType::Default => return Some(RustType::Tuple(vec![])), // -> ()
    };

    // Try to unwrap Result<ResponseValue<T>, Error<_>> → ResponseValue(T)
    if let Some(response_value_inner) = try_unwrap_progenitor_return(ty) {
        return Some(RustType::ResponseValue(Box::new(response_value_inner)));
    }

    // Fallback: parse as-is
    parse_rust_type(ty)
}

/// Try to unwrap `Result<ResponseValue<T>, Error<_>>` and return `T`.
fn try_unwrap_progenitor_return(ty: &Type) -> Option<RustType> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;

    // Must be Result<_, _>
    if seg.ident != "Result" {
        return None;
    }

    let result_inner = first_generic_arg(&seg.arguments)?;
    let Type::Path(rv_path) = result_inner else {
        return None;
    };
    let rv_seg = rv_path.path.segments.last()?;

    // Must be ResponseValue<T>
    if rv_seg.ident != "ResponseValue" {
        return None;
    }

    let inner = first_generic_arg(&rv_seg.arguments)?;
    parse_rust_type(inner)
}

// ── Utilities ────────────────────────────────────────────────────────────────

fn impl_target_name(imp: &ItemImpl) -> String {
    if let Type::Path(tp) = imp.self_ty.as_ref() {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident.to_string();
        }
    }
    String::new()
}

fn first_generic_arg(args: &PathArguments) -> Option<&Type> {
    if let PathArguments::AngleBracketed(ab) = args {
        for arg in &ab.args {
            if let GenericArgument::Type(ty) = arg {
                return Some(ty);
            }
        }
    }
    None
}

fn first_two_generic_args(args: &PathArguments) -> Option<(&Type, &Type)> {
    if let PathArguments::AngleBracketed(ab) = args {
        let mut types = ab.args.iter().filter_map(|arg| {
            if let GenericArgument::Type(ty) = arg {
                Some(ty)
            } else {
                None
            }
        });
        let first = types.next()?;
        let second = types.next()?;
        return Some((first, second));
    }
    None
}

fn collect_generic_args(args: &PathArguments) -> Vec<RustType> {
    if let PathArguments::AngleBracketed(ab) = args {
        ab.args
            .iter()
            .filter_map(|arg| {
                if let GenericArgument::Type(ty) = arg {
                    parse_rust_type(ty)
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec![]
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
