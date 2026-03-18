#[path = "codegen/mod.rs"]
mod codegen;

use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    // ── CallMessage factories (bullet_schema) ───────────────────────────────
    let call_msg_path = Path::new(&out_dir).join("call_message_factories.rs");

    let info = codegen::walk::extract_schema_info();
    let code = codegen::emit::emit_all(&info);
    fs::write(&call_msg_path, &code).expect("failed to write generated code");

    let total_variants: usize = info.action_groups.iter().map(|g| g.variants.len()).sum();
    println!(
        "cargo::warning=Generated {} factory methods across {} namespaces, {} struct wrappers, {} enums",
        total_variants,
        info.action_groups.len(),
        info.structs.len(),
        info.enums.len(),
    );

    // ── Progenitor type/client wrappers ─────────────────────────────────────
    let codegen_path = std::env::var("DEP_BULLET_RUST_CODEGEN_CODEGEN_PATH")
        .expect("DEP_BULLET_RUST_CODEGEN_CODEGEN_PATH not set — is bullet-rust-sdk a dependency?");

    let progenitor_info =
        codegen::progenitor::walk::extract_progenitor_info(Path::new(&codegen_path));
    let progenitor_code = codegen::progenitor::emit::emit_all(&progenitor_info);

    let progenitor_path = Path::new(&out_dir).join("progenitor_wrappers.rs");
    fs::write(&progenitor_path, &progenitor_code).expect("failed to write progenitor wrappers");

    println!(
        "cargo::warning=Generated {} progenitor type wrappers, {} enums, {} client methods",
        progenitor_info.structs.len(),
        progenitor_info.enums.len(),
        progenitor_info.methods.len(),
    );
}
