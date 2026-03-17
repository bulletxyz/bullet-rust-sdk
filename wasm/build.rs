#[path = "codegen/mod.rs"]
mod codegen;

use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("call_message_factories.rs");

    // Walk the schema.
    let info = codegen::walk::extract_schema_info();

    // Emit generated code.
    let code = codegen::emit::emit_all(&info);

    fs::write(&out_path, &code).expect("failed to write generated code");

    let total_variants: usize = info.action_groups.iter().map(|g| g.variants.len()).sum();
    println!(
        "cargo::warning=Generated {} factory methods across {} namespaces, {} struct wrappers, {} enums",
        total_variants,
        info.action_groups.len(),
        info.structs.len(),
        info.enums.len(),
    );
}
