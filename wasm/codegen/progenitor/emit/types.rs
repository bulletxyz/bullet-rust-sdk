//! Emit wasm-bindgen struct and enum wrappers.

use std::collections::{BTreeMap, HashSet};

use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::super::{EnumDetails, FieldDetails, FieldKind, RustType, StructDetails};
use super::type_map;

// ── SDK Path Helper ──────────────────────────────────────────────────────────

/// Build a fully-qualified SDK path from a module path.
///
/// Module paths are relative to the progenitor codegen root (e.g., `["types", "error"]`).
/// We emit `bullet_rust_sdk::codegen::<module_path>::<name>`.
fn sdk_qualified_path(module_path: &[String], name: &str) -> TokenStream {
    let name_ident = format_ident!("{}", name);

    // Build the full path by chaining segments with ::
    let mut tokens = quote! { bullet_rust_sdk::codegen };
    for seg in module_path {
        let seg_ident = format_ident!("{}", seg);
        tokens = quote! { #tokens::#seg_ident };
    }
    quote! { #tokens::#name_ident }
}

/// Check if derives contain Serialize.
fn has_serialize(derives: &[String]) -> bool {
    derives.iter().any(|d| d == "Serialize" || d.ends_with("::Serialize"))
}

// ── Struct emission ──────────────────────────────────────────────────────────

/// Emit a wrapper struct with getters for a progenitor type.
pub fn emit_struct(s: &StructDetails, enum_names: &HashSet<&str>) -> TokenStream {
    let sdk_type = sdk_qualified_path(&s.module_path, &s.name);
    let wrapper = format_ident!("Wasm{}", s.name);
    let js_name = type_map::js_name(&s.name);
    let serializable = has_serialize(&s.derives);

    // Newtype or empty struct → only expose toJSON (if serializable).
    if s.is_newtype || s.fields.is_empty() {
        let to_json_method = if serializable {
            quote! {
                #[wasm_bindgen(js_name = toJSON)]
                pub fn to_json(&self) -> String {
                    to_json(&self.0)
                }
            }
        } else {
            quote! {}
        };

        return quote! {
            #[wasm_bindgen(js_name = #js_name)]
            pub struct #wrapper(pub(crate) #sdk_type);

            #[wasm_bindgen(js_class = #js_name)]
            impl #wrapper {
                #to_json_method
            }
        };
    }

    let getters: Vec<TokenStream> = s.fields.iter().map(|f| emit_getter(f, enum_names)).collect();

    let to_json_method = if serializable {
        quote! {
            #[wasm_bindgen(js_name = toJSON)]
            pub fn to_json(&self) -> String {
                to_json(&self.0)
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper(pub(crate) #sdk_type);

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper {
            #to_json_method

            #(#getters)*
        }
    }
}

/// Emit a single getter method for a struct field.
fn emit_getter(f: &FieldDetails, enum_names: &HashSet<&str>) -> TokenStream {
    // Determine method name, JS property name, and field accessor based on field kind.
    let (method, js_name, field_accessor): (_, String, TokenStream) = match &f.kind {
        FieldKind::Named(name) => {
            let method = format_ident!("{}", name);
            let js_name = f.serde_rename.clone().unwrap_or_else(|| name.to_lower_camel_case());
            let accessor = quote! { #method };
            (method, js_name, accessor)
        }
        FieldKind::Index(i) => {
            let method = format_ident!("field_{}", i);
            let js_name = format!("field{}", i);
            let index = syn::Index::from(*i);
            let accessor = quote! { #index };
            (method, js_name, accessor)
        }
    };

    let method_str = method.to_string();
    let needs_js_attr = method_str != js_name;

    let (ret_ty, body) = type_map::getter_mapping(&f.ty, &field_accessor, enum_names);

    let attr = if needs_js_attr {
        quote! { #[wasm_bindgen(getter, js_name = #js_name)] }
    } else {
        quote! { #[wasm_bindgen(getter)] }
    };

    let js_ty = type_map::js_type_string(&f.ty, enum_names);
    let doc = format!("@returns {{{js_ty}}}");

    quote! {
        #[doc = #doc]
        #attr
        pub fn #method(&self) -> #ret_ty {
            #body
        }
    }
}

// ── Enum emission ────────────────────────────────────────────────────────────

/// Emit a wasm-bindgen C-style enum wrapper.
pub fn emit_enum(e: &EnumDetails) -> TokenStream {
    let sdk_type = sdk_qualified_path(&e.module_path, &e.name);
    let wrapper = format_ident!("Wasm{}", e.name);
    let js_name = type_map::js_name(&e.name);

    let variants: Vec<_> = e.variants.iter().map(|v| format_ident!("{}", v.name)).collect();
    let indices: Vec<_> = (0..e.variants.len()).map(|i| i as isize).collect();

    let arms: Vec<TokenStream> =
        variants.iter().map(|v| quote! { #wrapper::#v => #sdk_type::#v }).collect();

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        #[derive(Clone, Copy)]
        pub enum #wrapper {
            #(#variants = #indices,)*
        }

        impl #wrapper {
            pub fn into_domain(self) -> #sdk_type {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}

/// Emit an opaque wasm-bindgen wrapper for tagged/data enums.
///
/// Progenitor uses Rust enums with payload fields for schemas like `oneOf`
/// objects. wasm-bindgen cannot expose those variants directly, so expose the
/// value as a wrapper with JSON access while preserving the named TS type.
pub fn emit_data_enum(e: &EnumDetails) -> TokenStream {
    let sdk_type = sdk_qualified_path(&e.module_path, &e.name);
    let wrapper = format_ident!("Wasm{}", e.name);
    let js_name = type_map::js_name(&e.name);
    let serializable = has_serialize(&e.derives);
    let json_type = format!("{}Json", type_map::js_name(&e.name));
    let tag_getter = emit_data_enum_tag_getter(e, &sdk_type);
    let getters = emit_data_enum_field_getters(e, &sdk_type, &HashSet::new());

    let to_json_method = if serializable {
        quote! {
            #[wasm_bindgen(js_name = toJSON)]
            pub fn to_json(&self) -> String {
                to_json(&self.0)
            }

            #[wasm_bindgen(js_name = toObject, unchecked_return_type = #json_type)]
            pub fn to_object(&self) -> JsValue {
                serde_wasm_bindgen::to_value(&self.0).unwrap_or(JsValue::NULL)
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper(pub(crate) #sdk_type);

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper {
            #to_json_method

            #tag_getter

            #(#getters)*
        }
    }
}

pub fn emit_data_enum_typescript(e: &EnumDetails) -> TokenStream {
    let Some(ts) = data_enum_typescript(e) else {
        return quote! {};
    };
    let ident = custom_ts_ident(e);

    quote! {
        #[wasm_bindgen(typescript_custom_section)]
        const #ident: &'static str = #ts;
    }
}

fn custom_ts_ident(e: &EnumDetails) -> Ident {
    let name = format!("{}_TS_TYPES", e.name.to_snake_case().to_uppercase());
    format_ident!("{}", name)
}

fn data_enum_typescript(e: &EnumDetails) -> Option<String> {
    let tag = e.serde_tag.as_ref()?;
    let name = type_map::js_name(&e.name);
    let variants: Vec<String> = e
        .variants
        .iter()
        .map(|variant| {
            let tag_value = variant.serde_rename.as_deref().unwrap_or(&variant.name);
            let fields = variant
                .fields
                .iter()
                .filter_map(data_enum_ts_field)
                .map(|(name, ty)| format!("{name}: {ty}"))
                .collect::<Vec<_>>();

            let mut members = vec![format!("{tag}: \"{tag_value}\"")];
            members.extend(fields);
            format!("  | {{ {} }}", members.join("; "))
        })
        .collect();

    Some(format!(
        "export type {name}Json =\n{};\n",
        variants.join("\n")
    ))
}

fn data_enum_ts_field(field: &FieldDetails) -> Option<(String, String)> {
    let FieldKind::Named(name) = &field.kind else {
        return None;
    };
    let js_name = field.serde_rename.clone().unwrap_or_else(|| name.to_lower_camel_case());
    Some((js_name, json_ts_type(&field.ty)))
}

fn json_ts_type(ty: &RustType) -> String {
    match ty {
        RustType::String => "string".to_string(),
        RustType::Bool => "boolean".to_string(),
        RustType::Primitive(_) => "number".to_string(),
        RustType::Named { name, .. } if name == "Decimal" => "string".to_string(),
        RustType::Named { name, .. } if name == "Value" => "unknown".to_string(),
        RustType::Named { name, .. } => format!("{}Json", type_map::js_name(name)),
        RustType::Option(inner) => format!("{} | undefined", json_ts_type(inner)),
        RustType::Vec(inner) | RustType::Slice(inner) => format!("Array<{}>", json_ts_type(inner)),
        RustType::Map(_, _) => "Record<string, unknown>".to_string(),
        RustType::Ref(inner) | RustType::ResponseValue(inner) => json_ts_type(inner),
        RustType::Tuple(elems) if elems.is_empty() => "void".to_string(),
        _ => "unknown".to_string(),
    }
}

fn emit_data_enum_tag_getter(e: &EnumDetails, sdk_type: &TokenStream) -> TokenStream {
    let Some(tag) = &e.serde_tag else {
        return quote! {};
    };

    let method = format_ident!("{}", tag.to_snake_case());
    let needs_js_attr = method.to_string() != *tag;
    let attr = if needs_js_attr {
        quote! { #[wasm_bindgen(getter, js_name = #tag)] }
    } else {
        quote! { #[wasm_bindgen(getter)] }
    };

    let arms: Vec<TokenStream> = e
        .variants
        .iter()
        .map(|v| {
            let variant = format_ident!("{}", v.name);
            let tag_value = v.serde_rename.as_deref().unwrap_or(&v.name);
            if v.fields.is_empty() {
                quote! { #sdk_type::#variant => #tag_value.to_string() }
            } else {
                quote! { #sdk_type::#variant { .. } => #tag_value.to_string() }
            }
        })
        .collect();

    quote! {
        #[doc = "@returns {string}"]
        #attr
        pub fn #method(&self) -> String {
            match &self.0 {
                #(#arms,)*
            }
        }
    }
}

fn emit_data_enum_field_getters(
    e: &EnumDetails,
    sdk_type: &TokenStream,
    enum_names: &HashSet<&str>,
) -> Vec<TokenStream> {
    let mut fields: BTreeMap<String, FieldDetails> = BTreeMap::new();
    for variant in &e.variants {
        for field in &variant.fields {
            let FieldKind::Named(name) = &field.kind else {
                continue;
            };
            fields.entry(name.clone()).or_insert_with(|| field.clone());
        }
    }

    fields
        .values()
        .map(|field| emit_data_enum_field_getter(e, sdk_type, field, enum_names))
        .collect()
}

fn emit_data_enum_field_getter(
    e: &EnumDetails,
    sdk_type: &TokenStream,
    field: &FieldDetails,
    enum_names: &HashSet<&str>,
) -> TokenStream {
    let FieldKind::Named(name) = &field.kind else {
        return quote! {};
    };

    let method = format_ident!("{}", name);
    let js_name = field.serde_rename.clone().unwrap_or_else(|| name.to_lower_camel_case());
    let attr = if method.to_string() != js_name {
        quote! { #[wasm_bindgen(getter, js_name = #js_name)] }
    } else {
        quote! { #[wasm_bindgen(getter)] }
    };

    let ret_ty = type_map::wasm_type(&field.ty, enum_names);
    let js_ty = type_map::js_type_string(&RustType::Option(Box::new(field.ty.clone())), enum_names);
    let doc = format!("@returns {{{js_ty}}}");
    let binding = format_ident!("{}", name);
    let conv = data_enum_field_conversion(&field.ty, &quote! { #binding }, enum_names);

    let arms: Vec<TokenStream> = e
        .variants
        .iter()
        .map(|v| {
            let variant = format_ident!("{}", v.name);
            let has_field = v.fields.iter().any(|f| matches!(&f.kind, FieldKind::Named(n) if n == name));
            if has_field {
                quote! { #sdk_type::#variant { #binding, .. } => Some(#conv) }
            } else if v.fields.is_empty() {
                quote! { #sdk_type::#variant => None }
            } else {
                quote! { #sdk_type::#variant { .. } => None }
            }
        })
        .collect();

    quote! {
        #[doc = #doc]
        #attr
        pub fn #method(&self) -> Option<#ret_ty> {
            match &self.0 {
                #(#arms,)*
            }
        }
    }
}

fn data_enum_field_conversion(
    ty: &RustType,
    expr: &TokenStream,
    enum_names: &HashSet<&str>,
) -> TokenStream {
    match ty {
        RustType::String => quote! { #expr.clone() },
        RustType::Bool | RustType::Primitive(_) => quote! { *#expr },
        RustType::Map(_, _) => quote! { to_json(#expr) },
        RustType::Named { name, .. } if name == "Value" => quote! { to_json(#expr) },
        RustType::Named { name, .. } if enum_names.contains(name.as_str()) => {
            quote! { #expr.to_string() }
        }
        RustType::Named { name, .. } => {
            let w = format_ident!("Wasm{}", name);
            quote! { #w(#expr.clone()) }
        }
        RustType::Vec(inner) => {
            let conv = data_enum_field_conversion(inner, &quote! { v }, enum_names);
            quote! { #expr.iter().map(|v| #conv).collect() }
        }
        RustType::Option(inner) => {
            let conv = data_enum_field_conversion(inner, &quote! { v }, enum_names);
            quote! { #expr.as_ref().map(|v| #conv) }
        }
        _ => quote! { to_json(#expr) },
    }
}
