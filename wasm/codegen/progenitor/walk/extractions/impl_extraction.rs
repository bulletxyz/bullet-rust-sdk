use syn::{FnArg, ImplItem, ItemImpl, Pat};

use super::super::super::{ImplDetails, MethodDetails, ParamDetails};
use super::super::utils::{parse_return_type, parse_rust_type};

pub fn extract_impl(imp: &ItemImpl, target: &str, module_path: &[String]) -> ImplDetails {
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
