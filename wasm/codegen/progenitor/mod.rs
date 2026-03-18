//! Codegen for wasm-bindgen wrappers around progenitor-generated types and client methods.
//!
//! This module parses the progenitor-generated Rust code (via `syn`) and emits
//! wasm-bindgen wrapper types and client method implementations.

pub mod emit;
pub mod walk;

// ── Data types ───────────────────────────────────────────────────────────────

/// Complete extracted information from the progenitor-generated code.
#[derive(Debug)]
pub struct ProgenitorInfo {
    /// All struct types from the `types` module.
    pub structs: Vec<StructInfo>,
    /// All string enums from the `types` module.
    pub enums: Vec<EnumInfo>,
    /// All client methods from `impl Client`.
    pub methods: Vec<MethodInfo>,
}

/// A struct extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct StructInfo {
    /// The struct name, e.g. `Account`.
    pub name: String,
    /// Named fields. Empty for transparent newtypes.
    pub fields: Vec<FieldInfo>,
    /// Whether this is a newtype wrapper (single unnamed field, `#[serde(transparent)]`).
    pub is_newtype: bool,
}

/// A named field on a struct.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// Rust field name, e.g. `available_balance`.
    pub rust_name: String,
    /// JSON name from `#[serde(rename = "...")]`, if present.
    pub serde_rename: Option<String>,
    /// The field type.
    pub ty: FieldType,
}

/// Represents a parsed field type.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Bool,
    I32,
    I64,
    F64,
    /// `Option<T>`
    Option(Box<FieldType>),
    /// `Vec<T>`
    Vec(Box<FieldType>),
    /// A reference to another struct/enum in the `types` module.
    Ref(String),
    /// `serde_json::Map<String, serde_json::Value>` — serialise to JSON string.
    JsonMap,
    /// `serde_json::Value` — serialise to JSON string.
    JsonValue,
}

/// A string enum extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Enum name, e.g. `TxResult`.
    pub name: String,
    /// Variant names in PascalCase as progenitor outputs them, e.g. `Successful`.
    pub variants: Vec<String>,
}

/// A client method extracted from `impl Client`.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// Method name, e.g. `account_info`.
    pub name: String,
    /// Parameters (excluding `&self`), in order.
    pub params: Vec<ParamInfo>,
    /// The return type category.
    pub ret: ReturnKind,
}

/// A method parameter.
#[derive(Debug, Clone)]
pub struct ParamInfo {
    /// Parameter name, e.g. `symbol`.
    pub name: String,
    /// The parameter type.
    pub ty: ParamType,
}

/// Simplified parameter type (what progenitor actually uses).
#[derive(Debug, Clone)]
pub enum ParamType {
    /// `&str`
    Str,
    /// `Option<&str>`
    OptionStr,
    /// `i32`
    I32,
    /// `Option<i32>`
    OptionI32,
    /// `i64`
    I64,
    /// `Option<i64>`
    OptionI64,
    /// `&types::SomeType` — body parameter.
    BodyRef(String),
}

/// What a client method returns inside `ResponseValue<T>`.
#[derive(Debug, Clone)]
pub enum ReturnKind {
    /// Returns a single typed struct, e.g. `types::Account`.
    Schema(String),
    /// Returns `Vec<types::T>`.
    Array(String),
    /// Returns `ByteStream` (we expose as `String`).
    Stream,
    /// Returns `()`.
    Unit,
    /// Returns `serde_json::Map<String, serde_json::Value>` (we expose as JSON string).
    JsonMap,
}
