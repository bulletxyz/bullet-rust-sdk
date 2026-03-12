use serde_json::Value;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec_json = fetch_spec().unwrap_or_else(|| {
        println!("cargo::warning=Use cached `openapi.json` file.");
        include_str!("openapi.json").to_string()
    });
    // Unconditionally add a dependency here, so that one can manually fetch the json file.
    println!("cargo:rerun-if-changed=openapi.json");

    // Parse and apply workarounds
    let mut spec: Value = serde_json::from_str(&spec_json)?;

    // Convert OpenAPI 3.1 to 3.0 format
    // Change version from 3.1.0 to 3.0.0
    if let Some(openapi) = spec.get_mut("openapi") {
        *openapi = Value::String("3.0.0".to_string());
    }
    convert_nullable_types(&mut spec);
    fix_tuple_schemas(&mut spec);

    // Generate client code using progenitor
    let mut generator = progenitor::Generator::default();

    // Only keep 200 responses to simplify generated code
    filter_responses(&mut spec);

    let spec: openapiv3::OpenAPI = serde_json::from_value(spec.clone()).map_err(|e| {
        // Save the problematic spec for debugging
        let _ = std::fs::write(
            "openapi-debug.json",
            serde_json::to_string_pretty(&spec).unwrap(),
        );
        format!("Failed to parse OpenAPI spec: {e}. Saved debug output to openapi-debug.json")
    })?;
    let tokens = generator.generate_tokens(&spec)?;
    let ast = syn::parse2(tokens)?;
    let content = prettyplease::unparse(&ast);

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("codegen.rs");
    std::fs::write(&out_path, &content)?;
    Ok(())
}

/// Fix OpenAPI 3.1 tuple schemas (items: false + prefixItems)
/// Convert to OpenAPI 3.0 format by removing prefixItems and setting items to the first type
fn fix_tuple_schemas(v: &mut Value) {
    match v {
        Value::Object(map) => {
            // Check for tuple schema pattern: items: false + prefixItems
            if let Some(Value::Bool(false)) = map.get("items")
                && let Some(_prefix_items) = map.remove("prefixItems")
            {
                // For simplicity, use the first item type or make it generic
                // Since both elements are strings in our case, we can use string
                map.insert("items".to_string(), serde_json::json!({"type": "string"}));
            }

            // Recurse into all values
            for val in map.values_mut() {
                fix_tuple_schemas(val);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                fix_tuple_schemas(val);
            }
        }
        _ => {}
    }
}

/// Convert OpenAPI 3.1 nullable types to 3.0 format
/// OpenAPI 3.1 uses `type: ["string", "null"]` while 3.0 uses `type: "string", nullable: true`
fn convert_nullable_types(v: &mut Value) {
    match v {
        Value::Object(map) => {
            // Check if this is a type field with an array value
            if let Some(Value::Array(types)) = map.get_mut("type") {
                // Check if the array contains "null"
                if types.len() == 2 {
                    let has_null = types.iter().any(|t| t.as_str() == Some("null"));
                    if has_null {
                        // Find the non-null type
                        let actual_type =
                            types.iter().find(|t| t.as_str() != Some("null")).cloned();

                        if let Some(t) = actual_type {
                            // Replace array with single type and add nullable
                            map.insert("type".to_string(), t);
                            map.insert("nullable".to_string(), Value::Bool(true));
                        }
                    }
                }
            }
            // Check if this is a type field with a oneOf null value
            if let Some(Value::Array(types)) = map.get_mut("oneOf") {
                let has_null = types
                    .into_iter()
                    .any(|t| t.get("type").and_then(|x| x.as_str()) == Some("null"));
                if has_null {
                    types.retain(|t| t.get("type").and_then(|x| x.as_str()) != Some("null"));
                    map.insert("nullable".to_string(), Value::Bool(true));
                }
            }

            // Recurse into all values
            for val in map.values_mut() {
                convert_nullable_types(val);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                convert_nullable_types(val);
            }
        }
        _ => {}
    }
}

/// Filter responses to only include 200 status codes
fn filter_responses(spec: &mut Value) {
    if let Some(paths) = spec.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for path_item in paths.values_mut() {
            if let Some(path_obj) = path_item.as_object_mut() {
                for operation in path_obj.values_mut() {
                    if let Some(operation_obj) = operation.as_object_mut()
                        && let Some(responses) = operation_obj
                            .get_mut("responses")
                            .and_then(|r| r.as_object_mut())
                    {
                        responses.retain(|status_code, _| status_code == "200");
                    }
                }
            }
        }
    }
}

fn fetch_spec() -> Option<String> {
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");
    if std::env::var("CARGO_NET_OFFLINE").is_ok() {
        return None;
    }
    println!("cargo:rerun-if-env-changed=BULLET_API_ENDPOINT");
    let endpoint = std::env::var("BULLET_API_ENDPOINT")
        .unwrap_or_else(|_| "https://tradingapi.bullet.xyz".to_string());
    let url = endpoint + "/docs/rest/openapi.json";
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?
        .get(&url)
        .send()
        .ok()?;
    if response.status().is_success() {
        return response.text().ok();
    } else {
        println!(
            "cargo::warning=Spec fetch at '{url}' failed with: {}",
            response.status()
        );
    }
    None
}
