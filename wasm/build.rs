#[path = "codegen/mod.rs"]
mod codegen;

use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    let call_msg_path = Path::new(&out_dir).join("call_message_factories.rs");

    let info = codegen::walk::extract_schema_info();
    let code = codegen::emit::emit_all(&info);
    fs::write(&call_msg_path, &code).expect("failed to write generated code");

    let codegen_path = std::env::var("DEP_BULLET_RUST_CODEGEN_CODEGEN_PATH")
        .expect("DEP_BULLET_RUST_CODEGEN_CODEGEN_PATH not set — is bullet-rust-sdk a dependency?");

    let code_model = codegen::progenitor::walk::extract_code_model(Path::new(&codegen_path));
    let progenitor_code = codegen::progenitor::emit::emit_all(&code_model);

    let progenitor_path = Path::new(&out_dir).join("progenitor_wrappers.rs");
    fs::write(&progenitor_path, &progenitor_code).expect("failed to write progenitor wrappers");
}
