//! Schema-driven codegen for `CallMessage` factories: **walk** the `Transaction` schema into
//! wasm-bindgen param mappings, then **emit** Rust source.

pub mod emit;
pub mod walk;

use sov_universal_wallet::ty::{ContainerSerdeMetadata, Ty};

/// One CallMessage action group (User, Public, Keeper, Vault, Admin); emitted as a JS namespace
/// struct with one factory method per variant.
#[derive(Debug)]
pub struct ActionGroup {
    /// e.g. "User".
    pub call_message_variant: String,
    /// The nested action enum, e.g. "UserAction" in
    /// `CallMessage::User(UserAction::Deposit { .. })`.
    pub action_enum: String,
    pub variants: Vec<VariantInfo>,
}

/// A single variant within an action enum (e.g. UserAction::Deposit).
#[derive(Debug)]
pub struct VariantInfo {
    pub variant_name: String,
    pub fields: Vec<MappedField>,
}

/// A field with its resolved wasm-bindgen param type and conversion, e.g. `asset_id: u16` with
/// conversion `AssetId({v})`.
#[derive(Debug, Clone)]
pub struct MappedField {
    pub name: String,
    pub param_type: String,
    /// Converts the param into the domain type; `{v}` is the param name.
    pub conversion: String,
    /// JS-facing type override when the Rust boundary type loses detail.
    pub jsdoc_type: Option<String>,
    /// Optional params must be trailing in wasm-bindgen.
    pub is_optional: bool,
}

/// A schema struct used as an action parameter (e.g. `NewOrderArgs`); emitted as a `WasmXxx`
/// wrapper with a constructor.
#[derive(Debug)]
pub struct SchemaStruct {
    pub type_name: String,
    #[allow(dead_code)]
    pub schema_index: usize,
    pub fields: Vec<MappedField>,
}

/// A unit-only schema enum (e.g. `Side`); emitted as a C-style `WasmXxx` enum with `into_domain()`.
#[derive(Debug)]
pub struct SchemaEnum {
    pub type_name: String,
    pub schema_index: usize,
    pub variants: Vec<String>,
}

/// Output of walk, input to emit. `structs` and `enums` are the non-primitive types reachable from
/// `action_groups` fields; each gets a wrapper so JS can construct it.
#[derive(Debug)]
pub struct SchemaInfo {
    pub action_groups: Vec<ActionGroup>,
    pub structs: Vec<SchemaStruct>,
    pub enums: Vec<SchemaEnum>,
}

/// A raw schema field before mapping to [`MappedField`]. Exactly one of `schema_index` and
/// `primitive` is set.
#[derive(Debug, Clone)]
pub(crate) struct FieldInfo {
    pub name: String,
    pub schema_index: Option<usize>,
    pub primitive: Option<Primitive>,
}

/// Simplified primitive type (mirrors the schema `Primitive`).
#[derive(Debug, Clone)]
pub(crate) enum Primitive {
    Bool,
    ByteArray { len: usize },
    ByteVec,
    U8,
    U16,
    U32,
    U64,
    I16,
    I64,
    U128,
    String,
}

pub(crate) type Types = [Ty<sov_universal_wallet::schema::IndexLinking>];

pub(crate) type SerdeMetadata = [ContainerSerdeMetadata];
