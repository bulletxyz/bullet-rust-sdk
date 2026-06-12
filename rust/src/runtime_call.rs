//! SDK runtime-call envelope.
//!
//! `bullet-exchange-interface` 0.7 pins only the Exchange and Bank runtime
//! modules, while bridge withdrawals still need the legacy Warp module layout.
//! These types mirror the existing transaction envelope and add the narrow Warp
//! call the SDK exposes for withdrawal signing.

use std::fmt;
use std::str::FromStr;

use borsh::{BorshDeserialize, BorshSerialize};
use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::transaction::{Amount, TxDetails, UniquenessData, bank};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::CallMessage;

pub type BankCall = bank::CallMessage<Address>;

/// A decoded runtime call carried by a transaction.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum RuntimeCall {
    Bank(BankCall) = 2,
    Exchange(CallMessage) = 7,
    Warp(WarpCall) = 14,
}

impl RuntimeCall {
    pub fn exchange(call_message: CallMessage) -> Self {
        Self::Exchange(call_message)
    }

    pub fn warp_transfer_remote(args: WarpTransferRemoteArgs) -> Self {
        Warp::transfer_remote(args)
    }

    pub(crate) fn to_interface(
        &self,
    ) -> Option<bullet_exchange_interface::transaction::RuntimeCall> {
        match self {
            RuntimeCall::Bank(call) => {
                Some(bullet_exchange_interface::transaction::RuntimeCall::Bank(call.clone()))
            }
            RuntimeCall::Exchange(call) => {
                Some(bullet_exchange_interface::transaction::RuntimeCall::Exchange(call.clone()))
            }
            RuntimeCall::Warp(_) => None,
        }
    }
}

impl From<bullet_exchange_interface::transaction::RuntimeCall> for RuntimeCall {
    fn from(value: bullet_exchange_interface::transaction::RuntimeCall) -> Self {
        match value {
            bullet_exchange_interface::transaction::RuntimeCall::Bank(call) => Self::Bank(call),
            bullet_exchange_interface::transaction::RuntimeCall::Exchange(call) => {
                Self::Exchange(call)
            }
            _ => unreachable!("unsupported bullet-exchange-interface RuntimeCall variant"),
        }
    }
}

/// Warp module runtime calls.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum WarpCall {
    TransferRemote {
        warp_route: HexBytes32,
        destination_domain: u32,
        recipient: HexBytes32,
        amount: WarpAmount,
        relayer: Option<Address>,
        gas_payment_limit: WarpAmount,
    } = 4,
}

/// Namespace for constructing Warp runtime calls.
pub struct Warp;

impl Warp {
    pub fn transfer_remote(args: WarpTransferRemoteArgs) -> RuntimeCall {
        RuntimeCall::Warp(WarpCall::TransferRemote {
            warp_route: args.warp_route,
            destination_domain: args.destination_domain,
            recipient: args.recipient,
            amount: args.amount,
            relayer: args.relayer,
            gas_payment_limit: args.gas_payment_limit,
        })
    }
}

/// Arguments for `Warp.transfer_remote`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarpTransferRemoteArgs {
    pub warp_route: HexBytes32,
    pub amount: WarpAmount,
    pub destination_domain: u32,
    pub gas_payment_limit: WarpAmount,
    pub recipient: HexBytes32,
    pub relayer: Option<Address>,
}

/// A 32-byte value serialized as `0x`-prefixed hex in human-readable JSON.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, BorshDeserialize, BorshSerialize,
)]
pub struct HexBytes32(pub [u8; 32]);

impl HexBytes32 {
    pub fn to_hex(self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

impl FromStr for HexBytes32 {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let raw = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")).unwrap_or(value);
        if raw.chars().all(|c| c.is_ascii_hexdigit()) {
            let decoded = hex::decode(raw).map_err(|e| format!("invalid hex bytes32: {e}"))?;
            return match decoded.len() {
                32 => {
                    let mut out = [0u8; 32];
                    out.copy_from_slice(&decoded);
                    Ok(Self(out))
                }
                // Hyperlane EVM recipients are commonly provided as 20-byte
                // addresses and encoded as left-padded bytes32.
                20 => {
                    let mut out = [0u8; 32];
                    out[12..].copy_from_slice(&decoded);
                    Ok(Self(out))
                }
                len => Err(format!("expected 20 or 32 bytes, got {len}")),
            };
        }

        value
            .parse::<Address>()
            .map(|address| Self(address.0))
            .map_err(|_| format!("expected hex bytes32 or base58 address, got `{value}`"))
    }
}

impl fmt::Display for HexBytes32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for HexBytes32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_hex())
        } else {
            Serialize::serialize(&self.0, serializer)
        }
    }
}

impl<'de> Deserialize<'de> for HexBytes32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = <String as Deserialize>::deserialize(deserializer)?;
            value.parse().map_err(de::Error::custom)
        } else {
            <[u8; 32] as Deserialize>::deserialize(deserializer).map(Self)
        }
    }
}

/// A u128 runtime amount serialized as a decimal string in human-readable JSON.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
)]
pub struct WarpAmount(pub u128);

impl fmt::Display for WarpAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u128> for WarpAmount {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl From<WarpAmount> for Amount {
    fn from(value: WarpAmount) -> Self {
        Amount(value.0)
    }
}

impl FromStr for WarpAmount {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse::<u128>().map(Self).map_err(|e| format!("invalid u128 amount `{value}`: {e}"))
    }
}

impl Serialize for WarpAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.0.to_string())
        } else {
            Serialize::serialize(&self.0, serializer)
        }
    }
}

impl<'de> Deserialize<'de> for WarpAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(WarpAmountVisitor)
        } else {
            <u128 as Deserialize>::deserialize(deserializer).map(Self)
        }
    }
}

struct WarpAmountVisitor;

impl<'de> Visitor<'de> for WarpAmountVisitor {
    type Value = WarpAmount;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a u128 amount as a decimal string or integer")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
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
        Ok(WarpAmount(value as u128))
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(WarpAmount(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
            return Err(E::custom("amount number must be a non-negative integer"));
        }
        if value > u128::MAX as f64 {
            return Err(E::custom("amount exceeds u128"));
        }
        Ok(WarpAmount(value as u128))
    }
}

/// The unsigned transaction payload that gets signed.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
pub struct UnsignedTransactionPayload {
    pub runtime_call: RuntimeCall,
    pub uniqueness: UniquenessData,
    pub details: TxDetails,
}

impl UnsignedTransactionPayload {
    pub(crate) fn to_interface(
        &self,
    ) -> Option<bullet_exchange_interface::transaction::UnsignedTransaction> {
        Some(bullet_exchange_interface::transaction::UnsignedTransaction {
            runtime_call: self.runtime_call.to_interface()?,
            uniqueness: self.uniqueness.clone(),
            details: self.details.clone(),
        })
    }
}

/// A transaction with a single signer.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
pub struct Version0 {
    #[serde(with = "hex::serde")]
    pub signature: [u8; 64],
    #[serde(with = "hex::serde")]
    pub pub_key: [u8; 32],
    pub runtime_call: RuntimeCall,
    pub uniqueness: UniquenessData,
    pub details: TxDetails,
}

/// A signed transaction ready for submission.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum SignedTransaction {
    V0(Version0) = 0,
}

#[cfg(test)]
mod tests {
    use super::HexBytes32;

    #[test]
    fn hex_bytes32_accepts_uppercase_hex_prefix() {
        let parsed = format!("0X{}", "ab".repeat(32)).parse::<HexBytes32>().unwrap();

        assert_eq!(parsed.to_hex(), format!("0x{}", "ab".repeat(32)));
    }
}
