use syn::ItemStruct;

use super::super::super::{FieldDetails, FieldKind, StructDetails};
use super::super::utils::{
    extract_derives, extract_serde_rename, has_serde_transparent, parse_rust_type,
};

pub fn extract_struct(s: &ItemStruct, module_path: &[String]) -> Option<StructDetails> {
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
                        kind: FieldKind::Named(field_name),
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
                methods: Vec::new(),
            })
        }
        syn::Fields::Unnamed(unnamed) => {
            let fields = unnamed
                .unnamed
                .iter()
                .enumerate()
                .filter_map(|(i, f)| {
                    if !matches!(f.vis, syn::Visibility::Public(_)) {
                        return None;
                    }
                    let ty = parse_rust_type(&f.ty)?;
                    Some(FieldDetails {
                        kind: FieldKind::Index(i),
                        ty,
                        serde_rename: None,
                    })
                })
                .collect();

            Some(StructDetails {
                name,
                fields,
                is_newtype,
                module_path: module_path.to_vec(),
                derives,
                methods: Vec::new(),
            })
        }
        syn::Fields::Unit => None,
    }
}
