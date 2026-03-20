//! Codegen for wasm-bindgen wrappers around progenitor-generated types and client methods.
//!
//! This module parses the progenitor-generated Rust code (via `syn`) and emits
//! wasm-bindgen wrapper types and client method implementations.
//!
//! # Architecture
//!
//! ```text
//! progenitor codegen.rs
//!         │
//!         ▼
//!   walk/mod.rs  ──▶  CodeModel (RustType IR)
//!         │
//!         ▼
//!   emit/          ──▶  wasm-bindgen wrappers
//!     ├── type_map.rs   (single mapping point: RustType → WASM)
//!     ├── types.rs      (struct/enum wrappers)
//!     └── client.rs     (client method wrappers)
//! ```
//!
//! To add support for a new type, update `emit/type_map.rs`. Walk should
//! handle it automatically via the generic `RustType` representation.

pub mod emit;
pub mod walk;

use std::collections::HashMap;

// ── Code Model ───────────────────────────────────────────────────────────────

/// Complete code model extracted from progenitor-generated code.
///
/// This is the intermediate representation between syn parsing and WASM emit.
/// Walk produces this; emit consumes it.
#[derive(Debug)]
pub struct CodeModel {
    /// All types and impls, keyed by name.
    /// e.g. "Account" → Struct(...), "TxResult" → Enum(...), "Client" → Impl(...)
    pub items: HashMap<String, TypeInfo>,
}

/// A top-level item in the code model.
#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructDetails),
    Enum(EnumDetails),
    Impl(ImplDetails),
}

// ── RustType IR ──────────────────────────────────────────────────────────────

/// Simplified Rust type representation.
///
/// Sits between `syn::Type` (too detailed — lifetimes, spans, full paths) and
/// WASM output (too specific — wasm-bindgen types). Walk produces `RustType`;
/// emit maps it to WASM types via `type_map.rs`.
///
/// # Design decisions
///
/// - **Lifetimes are stripped.** They're irrelevant for WASM type mapping.
/// - **Named types store only the final path segment.** `types::Account` → "Account".
///   This assumes progenitor uses a flat `types` module with no name collisions.
/// - **Only structural types get promoted to variants.** `Option`, `Vec`, `Map`,
///   `Ref`, `Slice`, `Tuple`, `ResponseValue` each have their own variant because
///   they fundamentally change how emit generates code.
/// - **`ResponseValue<T>` is promoted** despite being progenitor-specific. It
///   appears on every client method return, so promoting it avoids repetitive
///   unwrapping in emit. Walk strips the outer `Result<..., Error<_>>` wrapper.
/// - **Everything else is `Named`** with optional generic args, letting emit
///   pattern match on the name (e.g., "ByteStream" for streaming responses).
#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    // ── Primitives ───────────────────────────────────────────────────────────
    Primitive(Primitive),
    String,
    Bool,

    // ── Structural types (affect codegen shape) ──────────────────────────────
    /// `Option<T>`
    Option(Box<RustType>),
    /// `Vec<T>`
    Vec(Box<RustType>),
    /// `(T1, T2, ...)` — empty vec for unit `()`
    Tuple(Vec<RustType>),
    /// `HashMap<K, V>` or `serde_json::Map<K, V>`
    Map(Box<RustType>, Box<RustType>),

    // ── References ───────────────────────────────────────────────────────────
    /// `&T` (lifetime stripped)
    Ref(Box<RustType>),
    /// `[T]` (slice type, usually behind a reference)
    Slice(Box<RustType>),

    // ── Progenitor-specific ──────────────────────────────────────────────────
    /// `ResponseValue<T>` — progenitor wraps all successful responses in this.
    ///
    /// Walk extracts `T` from `Result<ResponseValue<T>, Error<_>>` and stores
    /// it here. This is a deliberate departure from "pure shape" — we promote
    /// `ResponseValue` because it appears on every client method return, and
    /// stripping the `Result` wrapper in walk avoids repetitive unwrapping in emit.
    ResponseValue(Box<RustType>),

    // ── Everything else ──────────────────────────────────────────────────────
    /// Any named type with optional generic args.
    ///
    /// Examples:
    /// - `Account` → `Named { name: "Account", args: [] }`
    /// - `ByteStream` → `Named { name: "ByteStream", args: [] }`
    /// - `Foo<Bar>` → `Named { name: "Foo", args: [Named { name: "Bar", .. }] }`
    ///
    /// Emit pattern matches on `name` to handle special cases like `ByteStream`.
    Named {
        name: String,
        args: Vec<RustType>,
    },
}

/// Numeric primitive types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

// ── Struct Details ───────────────────────────────────────────────────────────

/// A struct extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct StructDetails {
    /// The struct name, e.g. `Account`.
    pub name: String,
    /// Named fields. Empty for newtypes.
    pub fields: Vec<FieldDetails>,
    /// Whether this is a newtype wrapper (single unnamed field, `#[serde(transparent)]`).
    pub is_newtype: bool,
    /// Module path relative to the codegen root, e.g. `["types", "error"]` for `types::error::ConversionError`.
    pub module_path: Vec<String>,
    /// Derive macros on this struct (e.g., `["Serialize", "Deserialize", "Clone"]`).
    pub derives: Vec<String>,
    /// Methods from inherent `impl` blocks (e.g. progenitor-generated accessors).
    pub methods: Vec<MethodDetails>,
}

/// A field on a struct (named or tuple).
#[derive(Debug, Clone)]
pub struct FieldDetails {
    /// Field identifier — named or positional index.
    pub kind: FieldKind,
    /// The field type.
    pub ty: RustType,
    /// JSON name from `#[serde(rename = "...")]`, if present.
    pub serde_rename: Option<String>,
}

/// How a field is accessed — by name or by tuple index.
#[derive(Debug, Clone)]
pub enum FieldKind {
    /// Named field, e.g. `balance` accessed as `.balance`.
    Named(String),
    /// Tuple struct field, e.g. index 0 accessed as `.0`.
    Index(usize),
}

// ── Enum Details ─────────────────────────────────────────────────────────────

/// An enum extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct EnumDetails {
    /// Enum name, e.g. `TxResult`.
    pub name: String,
    /// Variants.
    pub variants: Vec<VariantDetails>,
    /// Module path relative to the codegen root, e.g. `["types"]` for `types::TxStatus`.
    pub module_path: Vec<String>,
    /// Derive macros on this enum (e.g., `["Serialize", "Deserialize", "Clone"]`).
    pub derives: Vec<String>,
    /// Methods from inherent `impl` blocks (e.g. `as_str()` for string enums).
    pub methods: Vec<MethodDetails>,
}

/// An enum variant.
#[derive(Debug, Clone)]
pub struct VariantDetails {
    /// Variant name in PascalCase, e.g. `Successful`.
    pub name: String,
    /// Fields, if any. Empty for unit variants (C-style enums).
    /// Currently progenitor only generates unit variants for string enums.
    pub fields: Vec<FieldDetails>,
}

// ── Impl Details ─────────────────────────────────────────────────────────────

/// An impl block extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct ImplDetails {
    /// The type this impl is for, e.g. `Client`.
    pub target: String,
    /// Methods in this impl.
    pub methods: Vec<MethodDetails>,
    /// Module path relative to the codegen root.
    pub module_path: Vec<String>,
}

/// A method in an impl block.
#[derive(Debug, Clone)]
pub struct MethodDetails {
    /// Method name, e.g. `account_info`.
    pub name: String,
    /// Whether the method is async.
    pub is_async: bool,
    /// Parameters (excluding `&self`), in order.
    pub params: Vec<ParamDetails>,
    /// Return type. `None` for `-> ()` or no return type annotation.
    pub return_type: Option<RustType>,
}

/// A method parameter.
#[derive(Debug, Clone)]
pub struct ParamDetails {
    /// Parameter name, e.g. `symbol`.
    pub name: String,
    /// The parameter type.
    pub ty: RustType,
}
