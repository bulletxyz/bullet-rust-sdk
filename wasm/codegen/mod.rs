pub mod emitter;
pub mod schema_walker;
pub mod type_mapper;

/// One of the five CallMessage action groups (User, Public, Keeper, Vault, Admin).
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
#[derive(Debug)]
pub struct VariantInfo {
    /// The Rust variant name, e.g. "Deposit", "PlaceOrders".
    pub variant_name: String,
    /// Struct fields for this variant.
    pub fields: Vec<FieldInfo>,
}

/// A single field within a variant's struct.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// The field name as it appears in the Rust struct, e.g. "asset_id".
    pub name: String,
    /// The schema type index for this field's type.
    /// For `Link::ByIndex(n)`, this is `Some(n)`.
    /// For `Link::Immediate(prim)`, this is `None` and `primitive` is set.
    pub schema_index: Option<usize>,
    /// For `Link::Immediate(prim)`, the primitive type.
    pub primitive: Option<Primitive>,
}

/// Mirrors the schema `Primitive` but simplified for our codegen needs.
#[derive(Debug, Clone)]
pub enum Primitive {
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

/// A named struct type from the schema that needs a wasm-bindgen wrapper.
#[derive(Debug)]
pub struct SchemaStruct {
    /// The Rust type name, e.g. "NewOrderArgs".
    pub type_name: String,
    /// Schema index where this type lives.
    pub schema_index: usize,
    /// Fields of this struct.
    pub fields: Vec<FieldInfo>,
}

/// A simple enum (all unit variants) from the schema.
#[derive(Debug)]
pub struct SchemaEnum {
    /// The Rust type name, e.g. "Side".
    pub type_name: String,
    /// Schema index.
    pub schema_index: usize,
    /// Variant names, e.g. ["Bid", "Ask"].
    pub variants: Vec<String>,
}

/// Complete extracted schema info for codegen.
#[derive(Debug)]
pub struct SchemaInfo {
    /// The five CallMessage action groups.
    pub action_groups: Vec<ActionGroup>,
    /// All named structs reachable from CallMessage that need wrappers.
    pub structs: Vec<SchemaStruct>,
    /// All simple enums reachable from CallMessage.
    pub enums: Vec<SchemaEnum>,
}
