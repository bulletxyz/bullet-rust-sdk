//! Generic Rust syntax helpers for walk. Only `parse_return_type` is progenitor-aware.

use syn::{GenericArgument, PathArguments, ReturnType, Type};

use super::super::{Primitive, RustType};

/// Parse any `syn::Type` into the `RustType` IR; unknown types become `Named`.
pub fn parse_rust_type(ty: &Type) -> Option<RustType> {
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
        "bool" => Some(RustType::Bool),
        "String" | "str" => Some(RustType::String), // &str reference will wrap this
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
        // Cow<'_, T> -> T (e.g. Cow<'static, str> -> String)
        "Cow" => {
            let inner = first_generic_arg(&seg.arguments)?;
            parse_rust_type(inner)
        }

        _ => {
            let args = collect_generic_args(&seg.arguments);
            Some(RustType::Named { name: ident, args })
        }
    }
}

fn parse_reference(r: &syn::TypeReference) -> Option<RustType> {
    if let Type::Slice(s) = r.elem.as_ref() {
        let inner = parse_rust_type(&s.elem)?;
        return Some(RustType::Ref(Box::new(RustType::Slice(Box::new(inner)))));
    }

    let inner = parse_rust_type(&r.elem)?;
    Some(RustType::Ref(Box::new(inner)))
}

/// Parse a method return type, unwrapping `Result<ResponseValue<T>, Error<_>>`.
pub fn parse_return_type(ret: &ReturnType) -> Option<RustType> {
    let ty = match ret {
        ReturnType::Type(_, ty) => ty.as_ref(),
        ReturnType::Default => return Some(RustType::Tuple(vec![])), // -> ()
    };

    if let Some(response_value_inner) = try_unwrap_progenitor_return(ty) {
        return Some(RustType::ResponseValue(Box::new(response_value_inner)));
    }

    parse_rust_type(ty)
}

/// Try to unwrap `Result<ResponseValue<T>, Error<_>>` and return `T`.
fn try_unwrap_progenitor_return(ty: &Type) -> Option<RustType> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;

    if seg.ident != "Result" {
        return None;
    }

    let result_inner = first_generic_arg(&seg.arguments)?;
    let Type::Path(rv_path) = result_inner else {
        return None;
    };
    let rv_seg = rv_path.path.segments.last()?;

    if rv_seg.ident != "ResponseValue" {
        return None;
    }

    let inner = first_generic_arg(&rv_seg.arguments)?;
    parse_rust_type(inner)
}

pub fn first_generic_arg(args: &PathArguments) -> Option<&Type> {
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

pub fn has_serde_transparent(attrs: &[syn::Attribute]) -> bool {
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
/// Paths are kept, e.g. `serde::Serialize`.
pub fn extract_derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut derives = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        if let Ok(meta) = attr.meta.require_list() {
            meta.parse_nested_meta(|nested| {
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

pub fn extract_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        // Parse #[serde(rename = "camelCase")] — handles combined attributes like
        // #[serde(default, rename = "E", skip_serializing_if = "...")]
        let mut rename = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                rename = Some(s.value());
            }
            Ok(())
        });
        if rename.is_some() {
            return rename;
        }
    }
    None
}

pub fn impl_target_name(imp: &syn::ItemImpl) -> String {
    if let Type::Path(tp) = imp.self_ty.as_ref()
        && let Some(seg) = tp.path.segments.last()
    {
        return seg.ident.to_string();
    }
    String::new()
}
