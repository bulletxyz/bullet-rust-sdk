//! Parse progenitor-generated Rust code with `syn` and extract type/method information.

use std::path::Path;

use syn::{
    FnArg, GenericArgument, ImplItem, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct, Pat,
    PathArguments, ReturnType, Type,
};

use super::{
    EnumInfo, FieldInfo, FieldType, MethodInfo, ParamInfo, ParamType, ProgenitorInfo, ReturnKind,
    StructInfo,
};

/// Parse the progenitor-generated codegen.rs and extract all relevant information.
pub fn extract_progenitor_info(codegen_path: &Path) -> ProgenitorInfo {
    let source = std::fs::read_to_string(codegen_path).unwrap_or_else(|e| {
        panic!(
            "failed to read progenitor codegen at {}: {e}",
            codegen_path.display()
        )
    });

    let file = syn::parse_file(&source)
        .unwrap_or_else(|e| panic!("failed to parse progenitor codegen: {e}"));

    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut methods = Vec::new();

    for item in &file.items {
        match item {
            // The `types` module contains all struct/enum definitions.
            Item::Mod(module) if module_name(module) == "types" => {
                if let Some((_, items)) = &module.content {
                    for inner in items {
                        match inner {
                            Item::Struct(s) => {
                                if let Some(info) = extract_struct(s) {
                                    structs.push(info);
                                }
                            }
                            Item::Enum(e) => {
                                if let Some(info) = extract_enum(e) {
                                    enums.push(info);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            // `impl Client { ... }` contains all the REST methods.
            Item::Impl(imp) => {
                if impl_target_name(imp) == "Client" {
                    for item in &imp.items {
                        if let ImplItem::Fn(method) = item {
                            if let Some(info) = extract_method(method) {
                                methods.push(info);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    ProgenitorInfo {
        structs,
        enums,
        methods,
    }
}

// ── Struct extraction ────────────────────────────────────────────────────────

fn extract_struct(s: &ItemStruct) -> Option<StructInfo> {
    let name = s.ident.to_string();

    // Check for #[serde(transparent)] — marks newtype wrappers.
    let is_newtype = s.attrs.iter().any(|attr| {
        if !attr.path().is_ident("serde") {
            return false;
        }
        let Ok(nested) = attr.parse_args::<syn::Meta>() else {
            return false;
        };
        matches!(&nested, syn::Meta::Path(p) if p.is_ident("transparent"))
    });

    match &s.fields {
        syn::Fields::Named(named) => {
            let fields: Vec<FieldInfo> = named
                .named
                .iter()
                .filter_map(|f| {
                    let rust_name = f.ident.as_ref()?.to_string();
                    let serde_rename = extract_serde_rename(&f.attrs);
                    let ty = parse_field_type(&f.ty)?;
                    Some(FieldInfo {
                        rust_name,
                        serde_rename,
                        ty,
                    })
                })
                .collect();

            Some(StructInfo {
                name,
                fields,
                is_newtype,
            })
        }
        syn::Fields::Unnamed(_) => {
            // Tuple struct — treat as newtype with no exposed fields.
            Some(StructInfo {
                name,
                fields: vec![],
                is_newtype: true,
            })
        }
        syn::Fields::Unit => None,
    }
}

fn extract_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        // Parse #[serde(rename = "camelCase")]
        let nested: syn::MetaNameValue = attr.parse_args().ok()?;
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

// ── Enum extraction ──────────────────────────────────────────────────────────

fn extract_enum(e: &ItemEnum) -> Option<EnumInfo> {
    let name = e.ident.to_string();

    // Only extract simple enums (all unit variants, used as string enums).
    let all_unit = e.variants.iter().all(|v| v.fields.is_empty());
    if !all_unit {
        return None;
    }

    let variants: Vec<String> = e.variants.iter().map(|v| v.ident.to_string()).collect();

    Some(EnumInfo { name, variants })
}

// ── Method extraction ────────────────────────────────────────────────────────

fn extract_method(method: &syn::ImplItemFn) -> Option<MethodInfo> {
    let sig = &method.sig;

    // Only interested in public async methods.
    if sig.asyncness.is_none() {
        return None;
    }

    let name = sig.ident.to_string();
    // Skip constructors.
    if name == "new" || name == "new_with_client" || name == "api_version" {
        return None;
    }

    let mut params = Vec::new();
    for arg in &sig.inputs {
        if let FnArg::Typed(pat_ty) = arg {
            let param_name = match pat_ty.pat.as_ref() {
                Pat::Ident(id) => id.ident.to_string(),
                _ => continue,
            };
            let param_ty = parse_param_type(&pat_ty.ty)?;
            params.push(ParamInfo {
                name: param_name,
                ty: param_ty,
            });
        }
    }

    let ret = parse_return_type(&sig.output)?;

    Some(MethodInfo { name, params, ret })
}

// ── Type parsing ─────────────────────────────────────────────────────────────

/// Parse a struct field type into our FieldType model.
fn parse_field_type(ty: &Type) -> Option<FieldType> {
    match ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let ident = seg.ident.to_string();

            match ident.as_str() {
                "String" => Some(FieldType::String),
                "bool" => Some(FieldType::Bool),
                "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => Some(FieldType::I32),
                "i64" | "u64" => Some(FieldType::I64),
                "f32" | "f64" => Some(FieldType::F64),
                "Option" => {
                    let inner = first_generic_arg(&seg.arguments)?;
                    let inner_ty = parse_field_type(inner)?;
                    Some(FieldType::Option(Box::new(inner_ty)))
                }
                "Vec" => {
                    let inner = first_generic_arg(&seg.arguments)?;
                    let inner_ty = parse_field_type(inner)?;
                    Some(FieldType::Vec(Box::new(inner_ty)))
                }
                "Map" | "HashMap" => {
                    // serde_json::Map or std::collections::HashMap — serialize to JSON
                    Some(FieldType::JsonMap)
                }
                "Value" => {
                    // serde_json::Value
                    Some(FieldType::JsonValue)
                }
                other => {
                    // Check for fully-qualified paths like `std::string::String`.
                    if tp.path.segments.len() > 1 {
                        let full = path_to_string(&tp.path);
                        if full.ends_with("::String") || full == "std::string::String" {
                            return Some(FieldType::String);
                        }
                        if full.contains("serde_json::Map") {
                            return Some(FieldType::JsonMap);
                        }
                        if full.contains("serde_json::Value") {
                            return Some(FieldType::JsonValue);
                        }
                    }
                    // Assume it's a reference to another types module type.
                    Some(FieldType::Ref(other.to_string()))
                }
            }
        }
        _ => None,
    }
}

/// Parse a method parameter type.
fn parse_param_type(ty: &Type) -> Option<ParamType> {
    match ty {
        Type::Reference(r) => {
            // &str or &types::SomeType
            match r.elem.as_ref() {
                Type::Path(tp) => {
                    let last = tp.path.segments.last()?;
                    let ident = last.ident.to_string();
                    if ident == "str" {
                        Some(ParamType::Str)
                    } else {
                        // &types::SomeType — body param
                        Some(ParamType::BodyRef(ident))
                    }
                }
                _ => None,
            }
        }
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let ident = seg.ident.to_string();
            match ident.as_str() {
                "i32" => Some(ParamType::I32),
                "i64" => Some(ParamType::I64),
                "Option" => {
                    let inner = first_generic_arg(&seg.arguments)?;
                    match inner {
                        Type::Reference(r) => {
                            if let Type::Path(tp) = r.elem.as_ref() {
                                let last = tp.path.segments.last()?;
                                if last.ident == "str" {
                                    return Some(ParamType::OptionStr);
                                }
                            }
                            None
                        }
                        Type::Path(tp) => {
                            let last = tp.path.segments.last()?;
                            match last.ident.to_string().as_str() {
                                "i32" => Some(ParamType::OptionI32),
                                "i64" => Some(ParamType::OptionI64),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Parse the return type of a client method.
///
/// Progenitor methods return `Result<ResponseValue<T>, Error<()>>`.
/// We extract `T` from `ResponseValue<T>`.
fn parse_return_type(ret: &ReturnType) -> Option<ReturnKind> {
    let Type::Path(tp) = return_inner_type(ret)? else {
        return None;
    };

    // Dig into Result<ResponseValue<T>, ...> → ResponseValue<T> → T
    let result_seg = tp.path.segments.last()?;
    if result_seg.ident != "Result" {
        return None;
    }
    let rv_ty = first_generic_arg(&result_seg.arguments)?;
    let Type::Path(rv_path) = rv_ty else {
        return None;
    };
    let rv_seg = rv_path.path.segments.last()?;
    if rv_seg.ident != "ResponseValue" {
        return None;
    }
    let inner_ty = first_generic_arg(&rv_seg.arguments)?;

    classify_return_type(inner_ty)
}

fn classify_return_type(ty: &Type) -> Option<ReturnKind> {
    match ty {
        Type::Tuple(t) if t.elems.is_empty() => Some(ReturnKind::Unit),
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let ident = seg.ident.to_string();

            match ident.as_str() {
                "ByteStream" => Some(ReturnKind::Stream),
                "Vec" => {
                    let inner = first_generic_arg(&seg.arguments)?;
                    if let Type::Path(inner_tp) = inner {
                        let inner_name = inner_tp.path.segments.last()?.ident.to_string();
                        Some(ReturnKind::Array(inner_name))
                    } else {
                        None
                    }
                }
                "Map" => Some(ReturnKind::JsonMap),
                _ => {
                    // Check for types:: prefix
                    let type_name = if tp.path.segments.len() > 1 {
                        tp.path.segments.last()?.ident.to_string()
                    } else {
                        ident
                    };
                    Some(ReturnKind::Schema(type_name))
                }
            }
        }
        _ => None,
    }
}

// ── Utilities ────────────────────────────────────────────────────────────────

fn module_name(m: &ItemMod) -> String {
    m.ident.to_string()
}

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

fn return_inner_type(ret: &ReturnType) -> Option<&Type> {
    match ret {
        ReturnType::Type(_, ty) => Some(ty.as_ref()),
        ReturnType::Default => None,
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
