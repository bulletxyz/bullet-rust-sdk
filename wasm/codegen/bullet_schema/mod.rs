//! Schema-driven codegen for the Bullet exchange WASM SDK.
//!
//! Two phases:
//! - **walk**: traverse the `Transaction` schema, extract types, and resolve
//!   field mappings to wasm-bindgen-compatible params
//! - **emit**: generate Rust source code from the resolved data

pub mod emit;
pub mod walk;

use sov_universal_wallet::ty::Ty;

// ── Data types ───────────────────────────────────────────────────────────────

/// One of the five CallMessage action groups (User, Public, Keeper, Vault, Admin).
///
/// # Example
///
/// ```text
/// ActionGroup {
///     call_message_variant: "User",
///     action_enum: "UserAction",
///     variants: vec![
///         VariantInfo { variant_name: "Deposit", fields: vec![...] },
///         VariantInfo { variant_name: "Withdraw", fields: vec![...] },
///     ],
/// }
/// ```
///
/// This generates a namespace struct with factory methods:
///
/// ```ignore
/// #[wasm_bindgen]
/// pub struct User;
///
/// #[wasm_bindgen]
/// impl User {
///     #[wasm_bindgen(js_name = deposit)]
///     pub fn deposit(...) -> WasmResult<WasmCallMessage> {
///         Ok(WasmCallMessage {
///             inner: CallMessage::User(UserAction::Deposit { ... }),
///         })
///     }
/// }
/// ```
#[derive(Debug)]
pub struct ActionGroup {
    /// The CallMessage variant name: "User", "Vault", "Keeper", "Public", "Admin".
    pub call_message_variant: String,
    /// The Rust action enum name: "UserAction", "VaultAction", etc.
    pub action_enum: String,
    /// All variants within this action enum.
    pub variants: Vec<VariantInfo>,
}

/// A single variant within an action enum (e.g. UserAction::Deposit).
///
/// # Example
///
/// ```text
/// VariantInfo {
///     variant_name: "Deposit",
///     fields: vec![
///         MappedField { name: "asset_id", param_type: "u16", conversion: "AssetId({v})", ... },
///         MappedField { name: "amount", param_type: "String", conversion: "{v}.parse().unwrap()", ... },
///     ],
/// }
/// ```
#[derive(Debug)]
pub struct VariantInfo {
    /// The Rust variant name, e.g. "Deposit", "PlaceOrders".
    pub variant_name: String,
    /// Fields with resolved param mappings.
    pub fields: Vec<MappedField>,
}

/// A field with its resolved wasm-bindgen param type and conversion.
///
/// # Example
///
/// For a field `asset_id: AssetId` where `AssetId` is a newtype over `u16`:
///
/// ```text
/// MappedField {
///     name: "asset_id",
///     param_type: "u16",
///     conversion: "AssetId({v})",
///     is_optional: false,
/// }
/// ```
///
/// This generates: `fn foo(asset_id: u16) { ... AssetId(asset_id) ... }`
#[derive(Debug, Clone)]
pub struct MappedField {
    /// The field name as it appears in the Rust struct, e.g. "asset_id".
    pub name: String,
    /// The Rust type for the wasm-bindgen function parameter.
    pub param_type: String,
    /// The expression to convert the parameter into the domain type.
    /// Uses `{v}` as a placeholder for the parameter variable name.
    pub conversion: String,
    /// Whether this parameter is optional (must be trailing in wasm-bindgen).
    pub is_optional: bool,
}

/// A named struct from the schema that needs a wasm-bindgen wrapper.
///
/// # Example
///
/// ```text
/// SchemaStruct {
///     type_name: "NewOrderArgs",
///     schema_index: 42,
///     fields: vec![
///         MappedField { name: "market_id", param_type: "u16", conversion: "MarketId({v})", ... },
///         MappedField { name: "side", param_type: "WasmSide", conversion: "{v}.into_domain()", ... },
///         MappedField { name: "price", param_type: "String", conversion: "{v}.parse().unwrap()", ... },
///     ],
/// }
/// ```
///
/// This generates a wrapper struct with a constructor:
///
/// ```ignore
/// #[wasm_bindgen(js_name = NewOrderArgs)]
/// pub struct WasmNewOrderArgs {
///     pub(crate) inner: NewOrderArgs,
/// }
///
/// #[wasm_bindgen(js_class = NewOrderArgs)]
/// impl WasmNewOrderArgs {
///     #[wasm_bindgen(constructor)]
///     pub fn new(market_id: u16, side: WasmSide, price: String) -> WasmResult<WasmNewOrderArgs> {
///         Ok(WasmNewOrderArgs {
///             inner: NewOrderArgs {
///                 market_id: MarketId(market_id),
///                 side: side.into_domain(),
///                 price: price.parse().unwrap(),
///             },
///         })
///     }
/// }
/// ```
#[derive(Debug)]
pub struct SchemaStruct {
    /// The Rust type name, e.g. "NewOrderArgs".
    pub type_name: String,
    /// Schema index where this type lives.
    pub schema_index: usize,
    /// Fields with resolved param mappings.
    pub fields: Vec<MappedField>,
}

/// A simple enum (all unit variants) from the schema.
///
/// # Example
///
/// ```text
/// SchemaEnum {
///     type_name: "Side",
///     schema_index: 17,
///     variants: vec!["Bid", "Ask"],
/// }
/// ```
///
/// This generates a C-style wasm-bindgen enum with a conversion method:
///
/// ```ignore
/// #[wasm_bindgen(js_name = Side)]
/// #[derive(Clone, Copy)]
/// pub enum WasmSide {
///     Bid = 0,
///     Ask = 1,
/// }
///
/// impl WasmSide {
///     fn into_domain(self) -> Side {
///         match self {
///             WasmSide::Bid => Side::Bid,
///             WasmSide::Ask => Side::Ask,
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct SchemaEnum {
    /// The Rust type name, e.g. "Side".
    pub type_name: String,
    /// Schema index.
    pub schema_index: usize,
    /// Variant names, e.g. ["Bid", "Ask"].
    pub variants: Vec<String>,
}

/// Complete resolved schema info — output of walk, input to emit.
///
/// This is the bridge between the two phases:
/// - **walk** produces `SchemaInfo` by traversing the schema
/// - **emit** consumes `SchemaInfo` to generate Rust source code
#[derive(Debug)]
pub struct SchemaInfo {
    pub action_groups: Vec<ActionGroup>,
    pub structs: Vec<SchemaStruct>,
    pub enums: Vec<SchemaEnum>,
}

// ── Internal types (used during walk phase only) ─────────────────────────────

/// A raw field extracted from the schema, before mapping.
///
/// Either `schema_index` or `primitive` is set, never both.
/// This is the "unresolved" version that gets converted to [`MappedField`].
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
    U8,
    U16,
    U32,
    U64,
    I16,
    I64,
    U128,
    String,
}

/// Convenience alias for the schema type array.
pub(crate) type Types = [Ty<sov_universal_wallet::schema::IndexLinking>];
