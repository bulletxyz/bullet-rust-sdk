use syn::ItemEnum;

use super::super::super::{EnumDetails, FieldDetails, FieldKind, VariantDetails};
use super::super::utils::{extract_derives, extract_serde_rename, parse_rust_type};

pub fn extract_enum(e: &ItemEnum, module_path: &[String]) -> Option<EnumDetails> {
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
                            kind: FieldKind::Named(field_name),
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
                            kind: FieldKind::Index(i),
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
