//! Set an account's self-trade group.
//!
//! Accounts sharing a group are treated as a single entity by the exchange's
//! self-trade check, so they cannot trade against each other. Run this once
//! per account, signed by that account's owner key (a vault leader or a
//! sub-account's master may target the account via `TARGET_ADDRESS`; delegates
//! are refused by the exchange).
//!
//! # Usage
//!
//! ```bash
//! # Dry run (prints the plan, sends nothing):
//! PRIVATE_KEY='[1,2,...]' GROUP=<base58> \
//!   cargo run -p bullet-rust-sdk --example set_account_group
//!
//! # Send for real:
//! CONFIRM=yes PRIVATE_KEY=... GROUP=... \
//!   cargo run -p bullet-rust-sdk --example set_account_group
//! ```
//!
//! Environment:
//! - `PRIVATE_KEY` — signer secret key: 64 hex chars or a JSON byte array
//! - `GROUP` — base58 group address (any agreed address; conventionally one of
//!   the grouped accounts)
//! - `NETWORK` — `mainnet` (default), `testnet`, or a custom API URL
//! - `TARGET_ADDRESS` — optional base58 account to configure instead of the
//!   signer's own (a vault the signer leads, or a sub whose master signs)
//! - `SUB_ACCOUNT_INDEX` — optional; applies to the signer's own account and is
//!   ignored when `TARGET_ADDRESS` is set
//! - `CONFIRM` — must be `yes` to actually send

use bullet_rust_sdk::types::bullet_exchange_interface::address::Address;
use bullet_rust_sdk::{CallMessage, Client, Keypair, UserAction};

fn parse_keypair(raw: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        let bytes: Vec<u8> = serde_json::from_str(trimmed)?;
        let secret: [u8; 32] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| format!("PRIVATE_KEY must be 32 bytes, got {}", v.len()))?;
        Ok(Keypair::from_bytes(secret))
    } else {
        Ok(Keypair::from_hex(trimmed)?)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let keypair = parse_keypair(&std::env::var("PRIVATE_KEY")?)?;
    let group: Address = std::env::var("GROUP")?.parse()?;
    let network = std::env::var("NETWORK").unwrap_or_else(|_| "mainnet".to_string());
    let target: Option<Address> = match std::env::var("TARGET_ADDRESS") {
        Ok(addr) => Some(addr.parse()?),
        Err(_) => None,
    };
    let sub_account_index: Option<u8> = match std::env::var("SUB_ACCOUNT_INDEX") {
        Ok(index) => Some(index.parse()?),
        Err(_) => None,
    };

    let account = target.map_or_else(|| keypair.address(), |t| t.to_string());
    println!("network:  {network}");
    println!("signer:   {}", keypair.address());
    println!("account:  {account}");
    println!("group:    {group}");

    if std::env::var("CONFIRM").as_deref() != Ok("yes") {
        println!("\nDry run — set CONFIRM=yes to send.");
        return Ok(());
    }

    let client = Client::builder()
        .network(network)
        .keypair(keypair)
        .build()
        .await?;
    let response = client
        .send_call_message(CallMessage::User(UserAction::SetAccountGroup {
            address: target,
            group,
            sub_account_index,
        }))
        .await?;
    println!("\nSubmitted: {response:?}");
    Ok(())
}
