//! Transaction types, builder, and submission for WASM.
//!
//! All transaction construction goes through the builder pattern:
//!
//! ```js
//! // Build and send with explicit signer
//! const response = await Transaction.builder()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .signer(keypair)
//!     .send(client);
//!
//! // External signing
//! const unsigned = Transaction.builder()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .buildUnsigned(client);
//!
//! const signable = unsigned.toBytes();
//! const signature = myExternalSigner(signable);
//! const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
//!
//! // Submit later
//! await client.sendTransaction(signed);
//! ```

// The build-time codegen (`build.rs`) emits factory methods for *every*
// call-message variant, including ones deprecated in `bullet-exchange-interface`
// (e.g. `UserAction::WithdrawIso`, superseded by `Transfer`). Those variants must
// keep their factories for wire compatibility, so allow deprecated uses across
// this module rather than warning on generated code.
#![allow(deprecated)]

use std::fmt;
use std::str::FromStr;

use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::*;
use bullet_exchange_interface::time::UnixTimestampMicros;
use bullet_exchange_interface::transaction::{
    Amount, Gas, RuntimeCall, Transaction as RustSignedTransaction, WarpBytes32, warp,
};
use bullet_exchange_interface::types::{
    AdminType, AssetId, ClientOrderId, FeeTier, MarginDiscount, MarketId, MarketTradingStatus,
    OrderId, OrderType, Side, SpotCollateralTransferDirection, TokenId, TradingMode,
    TriggerDirection, TriggerOrderId, TriggerPriceCondition, TwapId,
};
use bullet_rust_sdk::types::CallMessage;
use bullet_rust_sdk::{
    SolanaLedgerTransaction as RustSolanaLedgerTransaction,
    SolanaOffchainTransaction as RustSolanaOffchainTransaction, Transaction as RustTransaction,
    UniquenessData, UnsignedTransaction,
};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::generated::WasmSubmitTxResponse;
use crate::keypair::WasmKeypair;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a base58 address string.
fn parse_addr(s: &str) -> Result<Address, String> {
    s.parse()
}

/// Parse an address-like relayer value from either Bullet base58 or hex bytes32.
fn parse_addr_like(s: &str) -> Result<Address, String> {
    s.parse::<Address>().or_else(|address_err| {
        parse_warp_bytes32(s)
            .map(|bytes| Address(bytes.0))
            .map_err(|bytes_err| format!("{address_err}; {bytes_err}"))
    })
}

/// Parse a 32-byte Warp value. Also accepts 20-byte EVM hex and left-pads it.
fn parse_warp_bytes32(value: &str) -> Result<WarpBytes32, String> {
    let raw = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return value
            .parse::<Address>()
            .map(|address| WarpBytes32(address.0))
            .map_err(|_| format!("expected hex bytes32 or base58 address, got `{value}`"));
    }

    let decoded = hex::decode(raw).map_err(|e| format!("invalid hex bytes32: {e}"))?;
    match decoded.len() {
        32 => {
            let mut out = [0u8; 32];
            out.copy_from_slice(&decoded);
            Ok(WarpBytes32(out))
        }
        20 => {
            let mut out = [0u8; 32];
            out[12..].copy_from_slice(&decoded);
            Ok(WarpBytes32(out))
        }
        len => Err(format!("expected 20 or 32 bytes, got {len}")),
    }
}

/// Parse a decimal string into `PositiveDecimal`.
fn parse_dec(s: &str) -> Result<PositiveDecimal, String> {
    PositiveDecimal::from_str(s).map_err(|e| format!("{e:?}"))
}

/// Parse a decimal string into `SurrogateDecimal` (used by funding/interest rate fields).
fn parse_surrogate_dec(
    s: &str,
) -> Result<bullet_exchange_interface::decimals::SurrogateDecimal, String> {
    use bullet_exchange_interface::decimals::SurrogateDecimal;
    SurrogateDecimal::from_str(s).map_err(|e| format!("{e:?}"))
}

/// Parse a JSON string into a serde-deserializable type.
fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

// ── WasmCallMessage ───────────────────────────────────────────────────────────

/// An opaque call message to be included in a transaction.
///
/// Construct via the namespace modules: `User`, `Public`, `Admin`, `Keeper`,
/// `Vault`, or `Warp`. Each module has static factory methods, e.g.
/// `User.deposit(0, "100.0")` or `Warp.transferRemote({...})`.
#[wasm_bindgen(js_name = CallMessage)]
pub struct WasmCallMessage {
    pub(crate) inner: RuntimeCall,
}

// ── Generated namespace structs (User, Public, Admin, Keeper, Vault) ─────────
//
// Each struct is a JS namespace with static factory methods that return
// `WasmCallMessage` instances. Generated from the Transaction schema by build.rs.
include!(concat!(env!("OUT_DIR"), "/call_message_factories.rs"));

// ── Warp namespace ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmWarpTransferRemoteArgs {
    #[serde(alias = "warp_route")]
    warp_route: String,
    amount: WasmWarpAmount,
    #[serde(alias = "destination_domain")]
    destination_domain: u32,
    #[serde(alias = "gas_payment_limit")]
    gas_payment_limit: WasmWarpAmount,
    recipient: String,
    relayer: Option<WasmRelayerInput>,
}

struct WasmWarpAmount(Amount);

const MAX_SAFE_JS_INTEGER: f64 = 9_007_199_254_740_991.0;

impl<'de> Deserialize<'de> for WasmWarpAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(WasmWarpAmountVisitor)
    }
}

struct WasmWarpAmountVisitor;

impl<'de> Visitor<'de> for WasmWarpAmountVisitor {
    type Value = WasmWarpAmount;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a u128 amount as a decimal string, bigint, or safe integer number")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value
            .parse::<u128>()
            .map(|value| WasmWarpAmount(Amount(value)))
            .map_err(E::custom)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(WasmWarpAmount(Amount(value as u128)))
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(WasmWarpAmount(Amount(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
            return Err(E::custom("amount number must be a non-negative integer"));
        }
        if value > MAX_SAFE_JS_INTEGER {
            return Err(E::custom(
                "amount number exceeds JavaScript safe integer range; pass a decimal string",
            ));
        }
        Ok(WasmWarpAmount(Amount(value as u128)))
    }
}

#[cfg(test)]
mod tests {
    use serde::de::Visitor as _;
    use serde::de::value::Error;

    use super::{MAX_SAFE_JS_INTEGER, WasmWarpAmountVisitor};

    #[test]
    fn warp_amount_rejects_unsafe_f64_numbers() {
        let err = match WasmWarpAmountVisitor.visit_f64::<Error>(MAX_SAFE_JS_INTEGER + 1.0) {
            Ok(_) => panic!("unsafe JS number should be rejected"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("safe integer"), "{err}");
    }

    #[test]
    fn warp_amount_accepts_safe_f64_numbers() {
        let amount = WasmWarpAmountVisitor
            .visit_f64::<Error>(MAX_SAFE_JS_INTEGER)
            .expect("safe JS number should be accepted");

        assert_eq!(amount.0.0, MAX_SAFE_JS_INTEGER as u128);
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WasmRelayerInput {
    Address(String),
    Tagged {
        #[serde(rename = "Standard", alias = "standard")]
        standard: Option<String>,
        #[serde(rename = "Vm", alias = "vm")]
        vm: Option<String>,
    },
}

impl WasmRelayerInput {
    fn into_address(self) -> Result<Address, String> {
        match self {
            WasmRelayerInput::Address(value) => parse_addr_like(&value),
            WasmRelayerInput::Tagged { standard, vm } => match (standard, vm) {
                (Some(_), Some(_)) => {
                    Err("relayer object must include either Standard or Vm, not both".to_string())
                }
                (Some(value), None) | (None, Some(value)) => parse_addr_like(&value),
                (None, None) => Err("relayer object must include Standard or Vm".to_string()),
            },
        }
    }
}

/// Warp bridge operations.
#[wasm_bindgen]
pub struct Warp;

#[wasm_bindgen]
impl Warp {
    /// Create a remote warp transfer runtime call.
    /// @param {{warpRoute: string, amount: string | number, destinationDomain: number,
    /// gasPaymentLimit: string | number, recipient: string, relayer?: {Standard: string} | {Vm:
    /// string} | string | null}} args - Transfer arguments. Use decimal strings for amounts
    /// above `Number.MAX_SAFE_INTEGER`. @returns {CallMessage}
    /// @example
    /// ```js
    /// const call = Warp.transferRemote({
    ///   warpRoute,
    ///   amount: "1000000",
    ///   destinationDomain: 1,
    ///   gasPaymentLimit: "400000",
    ///   recipient,
    ///   relayer: null,
    /// });
    /// const unsigned = Transaction.builder().callMessage(call).buildUnsigned(client);
    /// ```
    #[wasm_bindgen(js_name = transferRemote)]
    pub fn transfer_remote(args: JsValue) -> WasmResult<WasmCallMessage> {
        let args: WasmWarpTransferRemoteArgs =
            serde_wasm_bindgen::from_value(args).map_err(|e| e.to_string())?;
        let relayer = args
            .relayer
            .map(WasmRelayerInput::into_address)
            .transpose()?;

        Ok(WasmCallMessage {
            inner: RuntimeCall::Warp(warp::CallMessage::TransferRemote {
                warp_route: parse_warp_bytes32(&args.warp_route)
                    .map_err(|e| format!("warpRoute: {e}"))?,
                destination_domain: args.destination_domain,
                recipient: parse_warp_bytes32(&args.recipient)
                    .map_err(|e| format!("recipient: {e}"))?,
                amount: args.amount.0,
                relayer,
                gas_payment_limit: args.gas_payment_limit.0,
            }),
        })
    }
}

// ── Submit response helpers ─────────────────────────────────────────────────

#[wasm_bindgen(js_class = SubmitTxResponse)]
impl WasmSubmitTxResponse {
    /// Hyperlane message id emitted by a bridge withdrawal, when present.
    /// @returns {string | undefined}
    #[wasm_bindgen(getter, js_name = messageId)]
    pub fn message_id(&self) -> Option<String> {
        self.0.message_id()
    }
}

// ── WasmRuntimeCall ─────────────────────────────────────────────────────────

/// A whole runtime call — the single action a transaction carries.
///
/// This is the one input to [`Transaction.builder().call(...)`](WasmTransactionBuilder::call).
/// Construct it either from a typed call message or from JSON:
///
/// ```js
/// // Typed (the common case) — factories return a CallMessage:
/// const call = RuntimeCall.exchange(Admin.updateGlobalConfig(args));
///
/// // Dynamic / schema-driven — parse a serde JSON RuntimeCall
/// // (the `{ "exchange": { ... } }` / `{ "bank": { ... } }` tagged form,
/// // decimals as strings):
/// const call = RuntimeCall.fromJson(json);
///
/// Transaction.builder().call(call).buildUnsigned(client);
/// ```
#[wasm_bindgen(js_name = RuntimeCall)]
pub struct WasmRuntimeCall {
    pub(crate) inner: RuntimeCall,
}

#[wasm_bindgen(js_class = RuntimeCall)]
impl WasmRuntimeCall {
    /// Wrap a typed [`CallMessage`](WasmCallMessage) (e.g. the result of
    /// `Admin.updateGlobalConfig(...)` or `Warp.transferRemote(...)`) as a `RuntimeCall`.
    /// @param {CallMessage} call - The typed call message to wrap.
    /// @returns {RuntimeCall}
    pub fn exchange(call: WasmCallMessage) -> WasmRuntimeCall {
        WasmRuntimeCall { inner: call.inner }
    }

    /// Parse a `RuntimeCall` from its serde JSON representation.
    ///
    /// Accepts the tagged form (`{ "exchange": { ... } }` / `{ "bank": { ... } }`,
    /// decimals as strings). The JSON is validated against the runtime-call type
    /// here; it is validated against the connected chain's schema at build time.
    ///
    /// @param {string} json - JSON-serialized `RuntimeCall`.
    /// @returns {RuntimeCall}
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> WasmResult<WasmRuntimeCall> {
        let inner: RuntimeCall =
            serde_json::from_str(json).map_err(|e| format!("invalid RuntimeCall JSON: {e}"))?;
        Ok(WasmRuntimeCall { inner })
    }

    /// The `RuntimeCall` schema this SDK build supports, as JSON.
    ///
    /// Derived from the compiled `RuntimeCall` type via `sov_universal_wallet` —
    /// the same wire format the rollup's `/rollup/schema` endpoint uses, rooted
    /// at the `RuntimeCall` enum (`root_type_indices[0]`). It contains only the
    /// modules this SDK can actually build (Bank / Exchange / Warp), rather than
    /// every module the chain exposes, so consumers can drive a form/module
    /// selector straight off it with no need to filter the rollup's full set.
    ///
    /// @returns {string} JSON-serialized schema.
    pub fn schema() -> WasmResult<String> {
        use bullet_exchange_interface::schema::Schema;
        let schema = Schema::of_single_type::<RuntimeCall>()
            .map_err(|e| format!("failed to derive schema: {e:?}"))?;
        Ok(serde_json::to_string(&schema)
            .map_err(|e| format!("failed to serialize schema: {e}"))?)
    }
}

// ── WasmUnsignedTransaction ───────────────────────────────────────────────────

/// An unsigned transaction ready for external signing.
///
/// Created via `Transaction.builder().buildUnsigned(client)`. The chain hash
/// is already baked in, so `toBytes()` produces signable bytes directly.
///
/// ```js
/// const signable = unsigned.toBytes();
/// const display = unsigned.toDisplayMessage();
/// const signature = myExternalSigner(signable);
/// const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
/// ```
#[wasm_bindgen(js_name = UnsignedTransaction)]
pub struct WasmUnsignedTransaction {
    pub(crate) inner: UnsignedTransaction,
}

#[wasm_bindgen(js_class = UnsignedTransaction)]
impl WasmUnsignedTransaction {
    /// Serialize into the bytes that must be signed.
    ///
    /// Borsh-serializes the transaction and appends the chain hash (32 bytes)
    /// as domain separator. Pass the resulting `Uint8Array` to your signing function.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_bytes()?)
    }

    /// Reconstruct an `UnsignedTransaction` from the canonical bytes produced
    /// by `toBytes()` — the inverse of that method.
    ///
    /// Lets a coordinator (e.g. a multisig UI) persist a proposal as its exact
    /// signable bytes and rebuild the transaction later for display
    /// (`toDisplayMessage()`) and submission, without re-deriving it from a
    /// stored JSON representation. The rebuilt bytes are byte-identical to what
    /// was signed. `client` supplies the chain name and validates that the
    /// embedded chain hash matches the connected network.
    ///
    /// @param {Uint8Array} bytes - Bytes from a previous `toBytes()` call.
    /// @param {Client} client - The trading API client.
    /// @returns {UnsignedTransaction}
    /// @example
    /// ```js
    /// const unsigned = UnsignedTransaction.fromBytes(storedBytes, client);
    /// const display = unsigned.toDisplayMessage();
    /// const signableBytes = unsigned.toLedgerMultisigSignableBytes(config);
    /// ```
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(
        bytes: &[u8],
        client: &WasmTradingApi,
    ) -> WasmResult<WasmUnsignedTransaction> {
        Ok(WasmUnsignedTransaction {
            inner: UnsignedTransaction::from_bytes(bytes, &client.inner)?,
        })
    }

    /// Render the unsigned transaction payload as a human-readable message.
    ///
    /// Use this string in your own confirmation UI when an external wallet shows
    /// the raw Borsh bytes during `signMessage`. The wallet must still sign the
    /// bytes from `toBytes()`.
    ///
    /// @returns {string} Human-readable transaction payload for display.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// console.log(unsigned.toDisplayMessage());
    /// const signature = await wallet.signMessage(unsigned.toBytes());
    /// ```
    #[wasm_bindgen(js_name = toDisplayMessage)]
    pub fn to_display_message(&self) -> WasmResult<String> {
        Ok(self.inner.to_display_message()?)
    }

    /// Serialize into readable JSON bytes for offchain signing.
    ///
    /// External Solana wallets should sign these bytes when the backend uses
    /// the `solanaSimple` authenticator. The JSON includes `chain_name` and
    /// `chain_id`; the current sequencer offchain authenticator also requires
    /// the envelope assembled by `SolanaOffchainTransaction.fromParts(...)`.
    /// Submit the result with `client.sendOffChainTransaction(...)`.
    ///
    /// @returns {Uint8Array} UTF-8 JSON bytes to pass to `wallet.signMessage`.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// const message = unsigned.toMessageBytes();
    /// const signature = await wallet.signMessage(message);
    /// const pubKey = wallet.publicKey.toBytes();
    /// const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);
    /// await client.sendOffChainTransaction(tx);
    /// ```
    #[wasm_bindgen(js_name = toMessageBytes)]
    pub fn to_message_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_message_bytes()?)
    }

    /// Build the bytes a Ledger hardware wallet must sign.
    ///
    /// Returns the 85-byte Solana off-chain preamble (using the chain hash as
    /// application domain) concatenated with the JSON message bytes. Pass the
    /// result to `wallet.signMessage`; then assemble and submit with
    /// `SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature)` and
    /// `client.sendLedgerTransaction(tx)`.
    ///
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @returns {Uint8Array} Bytes to pass to `wallet.signMessage`.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// const pubKey = ledgerWallet.publicKey.toBytes();
    /// const signableBytes = unsigned.toLedgerSignableBytes(pubKey);
    /// const signature = await ledgerWallet.signMessage(signableBytes);
    /// const tx = SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature);
    /// await client.sendLedgerTransaction(tx);
    /// ```
    #[wasm_bindgen(js_name = toLedgerSignableBytes)]
    pub fn to_ledger_signable_bytes(&self, pub_key: &[u8]) -> WasmResult<Vec<u8>> {
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(self.inner.to_ledger_signable_bytes(&pub_key)?)
    }
}

// ── WasmSolanaLedgerTransaction ──────────────────────────────────────────────

/// A Solana offchain transaction using the spec-compliant Ledger wire format.
///
/// Use this for Ledger hardware wallets. Obtain bytes to sign from
/// `unsigned.toLedgerSignableBytes(pubKey)`, sign with the Ledger, then
/// assemble with `fromParts` and submit via `client.sendLedgerTransaction`.
#[wasm_bindgen(js_name = SolanaLedgerTransaction)]
pub struct WasmSolanaLedgerTransaction {
    pub(crate) inner: RustSolanaLedgerTransaction,
}

#[wasm_bindgen(js_class = SolanaLedgerTransaction)]
impl WasmSolanaLedgerTransaction {
    /// Assemble a Ledger transaction from an unsigned transaction, a 32-byte
    /// public key, and a 64-byte Ed25519 signature.
    ///
    /// @param {UnsignedTransaction} unsignedTx - The unsigned transaction.
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @param {Uint8Array} signature - 64-byte Ed25519 signature.
    /// @returns {SolanaLedgerTransaction}
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        pub_key: &[u8],
        signature: &[u8],
    ) -> WasmResult<WasmSolanaLedgerTransaction> {
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        Ok(WasmSolanaLedgerTransaction {
            inner: RustSolanaLedgerTransaction::from_parts(unsigned_tx.inner, pub_key, signature)?,
        })
    }

    /// Serialize to the raw binary wire format.
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Serialize to wire format and base64-encode.
    /// @returns {string}
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> String {
        self.inner.to_base64()
    }
}

// ── WasmSolanaOffchainTransaction ────────────────────────────────────────────

/// A Solana offchain transaction ready for submission.
///
/// Use this with external Solana wallets when you want the wallet to sign
/// readable JSON instead of Borsh bytes.
#[wasm_bindgen(js_name = SolanaOffchainTransaction)]
pub struct WasmSolanaOffchainTransaction {
    pub(crate) inner: RustSolanaOffchainTransaction,
}

#[wasm_bindgen(js_class = SolanaOffchainTransaction)]
impl WasmSolanaOffchainTransaction {
    /// Assemble a Solana offchain transaction from an unsigned transaction, a
    /// 64-byte Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing `unsigned.toMessageBytes()`.
    ///
    /// @param {UnsignedTransaction} unsignedTx - The unsigned transaction.
    /// @param {Uint8Array} signature - 64-byte Ed25519 signature.
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @returns {SolanaOffchainTransaction}
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        signature: &[u8],
        pub_key: &[u8],
    ) -> WasmResult<WasmSolanaOffchainTransaction> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(WasmSolanaOffchainTransaction {
            inner: RustSolanaOffchainTransaction::from_parts(
                unsigned_tx.inner,
                signature,
                pub_key,
            )?,
        })
    }

    /// Borsh-serialize the Solana offchain transaction to bytes.
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_bytes()?)
    }

    /// Borsh-serialize and base64-encode the Solana offchain transaction.
    /// @returns {string}
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        Ok(self.inner.to_base64()?)
    }
}

// ── WasmTransaction (SignedTransaction) ──────────────────────────────────────

/// A signed transaction ready for submission.
///
/// Passed directly to `Client.submitTransaction` or serialised to base64 via
/// `toBase64()` for WebSocket submission.
#[wasm_bindgen(js_name = SignedTransaction)]
pub struct WasmTransaction {
    pub(crate) inner: RustSignedTransaction,
}

#[wasm_bindgen(js_class = SignedTransaction)]
impl WasmTransaction {
    /// Assemble a signed transaction from an unsigned transaction, a 64-byte
    /// Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing the bytes from `unsigned.toBytes()`.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        signature: &[u8],
        pub_key: &[u8],
    ) -> WasmResult<WasmTransaction> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(WasmTransaction {
            inner: RustTransaction::from_parts(unsigned_tx.inner, signature, pub_key),
        })
    }

    /// Borsh-serialize the signed transaction to bytes.
    ///
    /// Useful for comparing two signed transactions byte-by-byte.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(RustTransaction::to_bytes(&self.inner)?)
    }

    /// Borsh-serialise and base64-encode the transaction.
    ///
    /// Use this when you need to pass the transaction over a WebSocket
    /// connection (e.g. `WebsocketHandle.orderPlace`).
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        Ok(RustTransaction::to_base64(&self.inner)?)
    }
}

// ── Transaction builder ──────────────────────────────────────────────────────

/// Transaction builder entry point.
///
/// Use `Transaction.builder()` to create a new builder, then chain
/// the required fields and call `.build(client)`, `.buildUnsigned(client)`,
/// or `.send(client)`.
#[wasm_bindgen(js_name = Transaction)]
pub struct WasmTransactionEntry;

#[wasm_bindgen(js_class = Transaction)]
impl WasmTransactionEntry {
    /// Create a new transaction builder.
    pub fn builder() -> WasmTransactionBuilder {
        WasmTransactionBuilder::new()
    }
}

/// Fluent builder for constructing and submitting transactions.
///
/// Created via `Transaction.builder()`.
///
/// # Required Fields
///
/// - `callMessage` - The action to execute (e.g., place order, withdraw)
///
/// # Optional Fields (fall back to client defaults if not set)
///
/// - `maxFee` - Maximum fee willing to pay (in base units)
/// - `priorityFeeBips` - Priority fee in basis points
/// - `gasLimit` - Optional gas limit [ref_time, proof_size]
/// - `generation` - Uniqueness generation value (default: current unix timestamp in milliseconds)
/// - `nonce` / `window` - Alternative uniqueness types (mutually exclusive with `generation`)
/// - `signer` - Keypair to sign the transaction (not required for `buildUnsigned`)
#[wasm_bindgen(js_name = TransactionBuilder)]
pub struct WasmTransactionBuilder {
    call: Option<WasmRuntimeCall>,
    max_fee: Option<u64>,
    priority_fee_bips: Option<u64>,
    gas_limit: Option<[u64; 2]>,
    uniqueness: Option<UniquenessData>,
    signer: Option<WasmKeypair>,
}

impl WasmTransactionBuilder {
    fn new() -> Self {
        WasmTransactionBuilder {
            call: None,
            max_fee: None,
            priority_fee_bips: None,
            gas_limit: None,
            uniqueness: None,
            signer: None,
        }
    }
}

#[wasm_bindgen(js_class = TransactionBuilder)]
impl WasmTransactionBuilder {
    /// Set the action for this transaction (required).
    ///
    /// Build a `RuntimeCall` with `RuntimeCall.exchange(typedCallMessage)` or
    /// `RuntimeCall.fromJson(json)`.
    /// @param {RuntimeCall} call - The runtime call (exchange action) to send.
    /// @returns {TransactionBuilder}
    pub fn call(mut self, call: WasmRuntimeCall) -> WasmTransactionBuilder {
        self.call = Some(call);
        self
    }

    /// Set the action from a typed call message.
    ///
    /// @deprecated Use `.call(RuntimeCall.exchange(msg))` instead. Kept as a
    /// convenience for the typed factories; equivalent to wrapping `msg` as an
    /// exchange `RuntimeCall`.
    #[wasm_bindgen(js_name = callMessage)]
    pub fn call_message(mut self, msg: WasmCallMessage) -> WasmTransactionBuilder {
        self.call = Some(WasmRuntimeCall { inner: msg.inner });
        self
    }

    /// Set the maximum fee (in base units) willing to pay for this transaction.
    #[wasm_bindgen(js_name = maxFee)]
    pub fn max_fee(mut self, fee: u64) -> WasmTransactionBuilder {
        self.max_fee = Some(fee);
        self
    }

    /// Set the priority fee in basis points.
    #[wasm_bindgen(js_name = priorityFeeBips)]
    pub fn priority_fee_bips(mut self, bips: u64) -> WasmTransactionBuilder {
        self.priority_fee_bips = Some(bips);
        self
    }

    /// Set the gas limit for this transaction.
    ///
    /// Takes [ref_time, proof_size] as parameters.
    #[wasm_bindgen(js_name = gasLimit)]
    pub fn gas_limit(mut self, ref_time: u64, proof_size: u64) -> WasmTransactionBuilder {
        self.gas_limit = Some([ref_time, proof_size]);
        self
    }

    /// Use window-based uniqueness with an explicit value.
    ///
    /// This is the default uniqueness mode (seeded with a microsecond unix
    /// timestamp when unset); call this only to pin a specific value. Window
    /// values must be unique per credential but need not be consecutive.
    /// Setting `nonce`/`generation`/`window` more than once keeps the last.
    /// @param {bigint} window - The window value to use.
    /// @returns {TransactionBuilder}
    pub fn window(mut self, window: u64) -> WasmTransactionBuilder {
        self.uniqueness = Some(UniquenessData::Window(window));
        self
    }

    /// Use generation-based uniqueness.
    ///
    /// Setting `nonce`/`generation`/`window` more than once keeps the last.
    /// @param {bigint} generation - The generation value to use.
    /// @returns {TransactionBuilder}
    pub fn generation(mut self, generation: u64) -> WasmTransactionBuilder {
        self.uniqueness = Some(UniquenessData::Generation(generation));
        self
    }

    /// Use nonce-based uniqueness (unique and consecutive per credential).
    ///
    /// Setting `nonce`/`generation`/`window` more than once keeps the last.
    /// @param {bigint} nonce - The credential nonce.
    /// @returns {TransactionBuilder}
    pub fn nonce(mut self, nonce: u64) -> WasmTransactionBuilder {
        self.uniqueness = Some(UniquenessData::Nonce(nonce));
        self
    }

    /// Set the keypair used to sign this transaction.
    pub fn signer(mut self, keypair: WasmKeypair) -> WasmTransactionBuilder {
        self.signer = Some(keypair);
        self
    }

    /// Build the unsigned transaction without signing.
    ///
    /// Returns an `UnsignedTransaction` that can be signed externally:
    ///
    /// ```js
    /// const unsigned = Transaction.builder()
    ///     .callMessage(callMsg)
    ///     .maxFee(10_000_000n)
    ///     .buildUnsigned(client);
    ///
    /// const signable = unsigned.toBytes();
    /// const signature = myExternalSigner(signable);
    /// const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
    /// ```
    #[wasm_bindgen(js_name = buildUnsigned)]
    pub fn build_unsigned(self, client: &WasmTradingApi) -> WasmResult<WasmUnsignedTransaction> {
        let call = self.call.ok_or("call is required (set via .call(...))")?;

        let max_fee = self
            .max_fee
            .map(|f| f as u128)
            .unwrap_or_else(|| client.inner.max_fee().0);
        let priority_fee_bips = self
            .priority_fee_bips
            .unwrap_or_else(|| client.inner.max_priority_fee_bips().0);
        let gas_limit = self.gas_limit.map(Gas).or_else(|| client.inner.gas_limit());

        let unsigned = UnsignedTransaction::from_runtime_call(
            call.inner,
            max_fee,
            priority_fee_bips,
            gas_limit,
            self.uniqueness,
            &client.inner,
        )?;

        Ok(WasmUnsignedTransaction { inner: unsigned })
    }

    /// Build the signed transaction without sending it.
    pub fn build(self, client: &WasmTradingApi) -> WasmResult<WasmTransaction> {
        let call = self.call.ok_or("call is required (set via .call(...))")?;

        let max_fee = self.max_fee.map(|f| f as u128);
        let gas_limit = self.gas_limit.map(Gas);
        let signer_ref = self.signer.as_ref().map(|s| &s.inner);

        let signed = RustTransaction::from_runtime_call(
            call.inner,
            max_fee,
            self.priority_fee_bips,
            gas_limit,
            self.uniqueness,
            signer_ref,
            &client.inner,
        )?;

        Ok(WasmTransaction { inner: signed })
    }

    /// Sign and submit the transaction to the network.
    /// @param {Client} client - The trading API client.
    /// @returns {Promise<SubmitTxResponse>}
    pub async fn send(
        self,
        client: &WasmTradingApi,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let tx = self.build(client)?;
        client.send_transaction(&tx).await
    }
}

impl Default for WasmTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Client convenience methods ───────────────────────────────────────────────

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Send a signed transaction to the network via REST.
    /// @param {Transaction} tx - A signed transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendTransaction)]
    pub async fn send_transaction(
        &self,
        tx: &WasmTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Send a Solana offchain transaction to the sequencer via REST.
    /// @param {SolanaOffchainTransaction} tx - A signed Solana offchain transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendOffChainTransaction)]
    pub async fn send_off_chain_transaction(
        &self,
        tx: &WasmSolanaOffchainTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_offchain_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Send a Ledger-signed transaction to the sequencer via REST.
    ///
    /// Use this with Ledger hardware wallets. Sign with
    /// `unsigned.toLedgerSignableBytes(pubKey)`, then assemble via
    /// `SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature)`.
    ///
    /// @param {SolanaLedgerTransaction} tx - A signed Ledger transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendLedgerTransaction)]
    pub async fn send_ledger_transaction(
        &self,
        tx: &WasmSolanaLedgerTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_ledger_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Sign and submit a call message in one step.
    ///
    /// This is a convenience method that wraps
    /// `Transaction.builder().callMessage(msg).send(client)` into a single call using the
    /// client's default keypair, max fee, and gas settings.
    ///
    /// @param {CallMessage} msg - A call message (e.g. from `User.placeOrders(...)`)
    /// @returns {Promise<SubmitTxResponse>}
    ///
    /// @example
    /// ```js
    /// const order = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false);
    /// const resp = await client.sendCallMessage(User.placeOrders(0, [order], false));
    /// ```
    #[wasm_bindgen(js_name = sendCallMessage)]
    pub async fn send_call_message(
        &self,
        msg: WasmCallMessage,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_runtime_call(msg.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }
}
