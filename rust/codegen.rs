use std::collections::BTreeSet;

use syn::{Arm, Expr, File, ImplItem, Item, Lit, Pat, RangeLimits, Stmt, Type, parse_quote};

pub fn merge_response_arms(file: &mut File) {
    let scrutinee: Expr = parse_quote!(response.status().as_u16());
    for item in &mut file.items {
        let Item::Impl(item) = item else { continue };
        if item.trait_.is_some()
            || !matches!(item.self_ty.as_ref(), Type::Path(path) if path.qself.is_none() && path.path.is_ident("Client"))
        {
            continue;
        }
        for method in &mut item.items {
            let ImplItem::Fn(method) = method else {
                continue;
            };
            let Some(Stmt::Expr(Expr::Match(status), None)) = method.block.stmts.last_mut() else {
                continue;
            };
            if *status.expr != scrutinee {
                continue;
            }
            let mut arms: Vec<Arm> = Vec::with_capacity(status.arms.len());
            for arm in status.arms.drain(..) {
                if let Some(previous) = arms.last_mut()
                    && previous.attrs.is_empty()
                    && arm.attrs.is_empty()
                    && previous.body == arm.body
                    && let Some(mut statuses) = error_statuses(&previous.pat)
                    && let Some(next) = error_statuses(&arm.pat)
                {
                    statuses.extend(next);
                    let first = *statuses.first().expect("error statuses are nonempty");
                    let last = *statuses.last().expect("error statuses are nonempty");
                    let left = &previous.pat;
                    let right = &arm.pat;
                    previous.pat = if first == last {
                        parse_quote!(#first)
                    } else if (first..=last).count() == statuses.len() {
                        parse_quote!(#first..=#last)
                    } else {
                        parse_quote!(#left | #right)
                    };
                } else {
                    arms.push(arm);
                }
            }
            status.arms = arms;
        }
    }
}

fn error_statuses(pattern: &Pat) -> Option<BTreeSet<u16>> {
    // Syn 3 puts match guards in Pat::Guard, which must not contribute statuses.
    match pattern {
        Pat::Lit(lit) => Some(BTreeSet::from([error_status(&lit.lit)?])),
        Pat::Range(range) if matches!(range.limits, RangeLimits::Closed(_)) => {
            let Expr::Lit(start) = range.start.as_deref()? else {
                return None;
            };
            let Expr::Lit(end) = range.end.as_deref()? else {
                return None;
            };
            let (start, end) = (error_status(&start.lit)?, error_status(&end.lit)?);
            (start <= end).then(|| (start..=end).collect())
        }
        Pat::Or(pattern) => pattern
            .cases
            .iter()
            .try_fold(BTreeSet::new(), |mut statuses, case| {
                statuses.extend(error_statuses(case)?);
                Some(statuses)
            }),
        _ => None,
    }
}

fn error_status(literal: &Lit) -> Option<u16> {
    let Lit::Int(literal) = literal else {
        return None;
    };
    let value = literal.base10_parse().ok()?;
    (400..=599).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use prettyplease::unparse;
    use syn::parse_str;

    use super::*;

    #[test]
    fn merges_only_identical_numeric_error_responses_without_widening() {
        for (arms, expected) in [
            (
                "400 => error(), 500 => error(), _ => unknown()",
                "400 | 500 => error(), _ => unknown()",
            ),
            (
                "400..=499 => error(), 500..=599 => error(), _ => unknown()",
                "400u16..=599u16 => error(), _ => unknown()",
            ),
            (
                "400 => error(), 402 => error(), _ => unknown()",
                "400 | 402 => error(), _ => unknown()",
            ),
            (
                "400 => error(), 401 => error(), 403 => error()",
                "400u16..=401u16 | 403 => error()",
            ),
            (
                "400 => error(), 401 => error(), 402 => error()",
                "400u16..=402u16 => error()",
            ),
            (
                "400 => error(), 500 => other()",
                "400 => error(), 500 => other()",
            ),
            (
                "200 => error(), 400 => error()",
                "200 => error(), 400 => error()",
            ),
            (
                "400 if enabled => error(), 500 => error()",
                "400 if enabled => error(), 500 => error()",
            ),
            (
                "400 => error(), 500 if enabled => error()",
                "400 => error(), 500 if enabled => error()",
            ),
            (
                "400 if first => error(), 500 if second => error()",
                "400 if first => error(), 500 if second => error()",
            ),
            (
                "400 | 401 if enabled => error(), 500 => error()",
                "400 | 401 if enabled => error(), 500 => error()",
            ),
            (
                "#[cfg(feature = \"special\")] 400 => error(), 500 => error()",
                "#[cfg(feature = \"special\")] 400 => error(), 500 => error()",
            ),
        ] {
            let source = |arms: &str| {
                format!(
                    "impl Client {{ fn request() {{ match response.status().as_u16() {{ {arms} }} }} }}"
                )
            };
            let mut actual: File = parse_str(&source(arms)).unwrap();
            let expected: File = parse_str(&source(expected)).unwrap();
            merge_response_arms(&mut actual);
            assert_eq!(unparse(&actual), unparse(&expected), "{arms}");
            let normalized = actual.clone();
            merge_response_arms(&mut actual);
            assert_eq!(actual, normalized, "normalization must be idempotent");
        }
        for source in [
            "impl Other { fn request() { match response.status().as_u16() { 400 => error(), 500 => error() } } }",
            "impl Client { fn request() { match other.status().as_u16() { 400 => error(), 500 => error() } } }",
        ] {
            let mut actual: File = parse_str(source).unwrap();
            let original = actual.clone();
            merge_response_arms(&mut actual);
            assert_eq!(actual, original);
        }
    }
}
