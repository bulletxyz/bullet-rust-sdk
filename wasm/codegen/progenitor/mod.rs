//! wasm-bindgen wrappers for progenitor-generated types and client methods.
//!
//! ```text
//! progenitor codegen.rs --walk (syn)--> CodeModel (RustType IR) --emit--> wasm-bindgen wrappers
//! ```
//!
//! New types only need a mapping in `emit/type_map.rs`.

pub mod emit;
pub mod walk;

use std::collections::BTreeMap;

/// Code model extracted from progenitor output; walk produces it, emit consumes it.
#[derive(Debug)]
pub struct CodeModel {
    /// e.g. "Account" -> Struct, "TxResult" -> Enum, "Client" -> Impl.
    pub items: BTreeMap<String, TypeInfo>,
}

/// A top-level item in the code model.
#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructDetails),
    Enum(EnumDetails),
    Impl(ImplDetails),
}

/// Simplified Rust type between `syn::Type` and WASM output. Lifetimes are stripped and named
/// types keep only their last path segment (assumes progenitor's flat `types` module). Only
/// shape-changing types get their own variant; everything else is `Named`.
#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    Primitive(Primitive),
    String,
    Bool,

    Option(Box<RustType>),
    Vec(Box<RustType>),
    /// `(T1, T2, ...)` — empty vec for unit `()`
    Tuple(Vec<RustType>),
    /// `HashMap<K, V>` or `serde_json::Map<K, V>`
    Map(Box<RustType>, Box<RustType>),

    Ref(Box<RustType>),
    Slice(Box<RustType>),

    /// `ResponseValue<T>`, unwrapped by walk from `Result<ResponseValue<T>, Error<_>>` since every
    /// client method returns it.
    ResponseValue(Box<RustType>),

    /// Any other named type, e.g. `Foo<Bar>` -> `Named { name: "Foo", args: [..] }`. Emit matches
    /// on `name` for special cases like `ByteStream`.
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

/// A struct extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct StructDetails {
    pub name: String,
    /// Empty for newtypes.
    pub fields: Vec<FieldDetails>,
    /// Single unnamed field, `#[serde(transparent)]`.
    pub is_newtype: bool,
    /// Relative to the codegen root, e.g. `["types", "error"]`.
    pub module_path: Vec<String>,
    pub derives: Vec<String>,
    /// Inherent methods (e.g. progenitor-generated accessors).
    pub methods: Vec<MethodDetails>,
}

/// A field on a struct (named or tuple).
#[derive(Debug, Clone)]
pub struct FieldDetails {
    pub kind: FieldKind,
    pub ty: RustType,
    /// From `#[serde(rename = "...")]`.
    pub serde_rename: Option<String>,
}

/// How a field is accessed — by name or by tuple index.
#[derive(Debug, Clone)]
pub enum FieldKind {
    Named(String),
    Index(usize),
}

/// An enum extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct EnumDetails {
    pub name: String,
    pub variants: Vec<VariantDetails>,
    /// Relative to the codegen root, e.g. `["types"]`.
    pub module_path: Vec<String>,
    pub derives: Vec<String>,
    /// Inherent methods (e.g. `as_str()` for string enums).
    pub methods: Vec<MethodDetails>,
}

/// An enum variant.
#[derive(Debug, Clone)]
pub struct VariantDetails {
    pub name: String,
    /// Empty for unit variants (progenitor only generates those, for string enums).
    pub fields: Vec<FieldDetails>,
}

/// An impl block extracted from progenitor output.
#[derive(Debug, Clone)]
pub struct ImplDetails {
    #[allow(dead_code)]
    pub target: String,
    pub methods: Vec<MethodDetails>,
    #[allow(dead_code)]
    pub module_path: Vec<String>,
}

/// A method in an impl block.
#[derive(Debug, Clone)]
pub struct MethodDetails {
    pub name: String,
    pub is_async: bool,
    /// Excluding `&self`.
    pub params: Vec<ParamDetails>,
    /// `None` for `()`.
    pub return_type: Option<RustType>,
}

/// A method parameter.
#[derive(Debug, Clone)]
pub struct ParamDetails {
    pub name: String,
    pub ty: RustType,
}
