use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::*;
use bullet_exchange_interface::time::UnixTimestampMicros;
use bullet_exchange_interface::transaction::Transaction;
use bullet_exchange_interface::types::{
    AdminType, AssetId, FeeTier, MarketId, OrderId, TriggerOrderId, TwapId,
};
use bullet_rust_sdk::types::CallMessage;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a base58 address string.
fn parse_addr(s: &str) -> Result<Address, String> {
    s.parse()
}

/// Parse a decimal string into `PositiveDecimal`.
fn parse_dec(s: &str) -> Result<PositiveDecimal, String> {
    PositiveDecimal::from_str(s).map_err(|e| format!("{e:?}"))
}

/// Parse a JSON string into a serde-deserializable type.
fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

// ── WasmCallMessage ───────────────────────────────────────────────────────────

/// A call message to be included in a transaction.
///
/// Construct via the static factory methods on this class, one per
/// operation variant.  `fromJSON` is available as an escape hatch for
/// any variant not yet covered by a typed factory.
#[wasm_bindgen(js_name = CallMessage)]
pub struct WasmCallMessage {
    pub(crate) inner: CallMessage,
}

#[wasm_bindgen(js_class = CallMessage)]
impl WasmCallMessage {
    /// Escape hatch: parse an arbitrary JSON-encoded `CallMessage`.
    #[wasm_bindgen(js_name = fromJSON)]
    pub fn from_json_raw(json: &str) -> WasmResult<WasmCallMessage> {
        let inner: CallMessage = serde_json::from_str(json)?;
        Ok(WasmCallMessage { inner })
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Public (permissionless) operations
    // ═════════════════════════════════════════════════════════════════════════

    /// Liquidate perp positions for an underwater account.
    #[wasm_bindgen(js_name = liquidatePerpPositions)]
    pub fn liquidate_perp_positions(address: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::LiquidatePerpPositions {
                address: parse_addr(address)?,
            }),
        })
    }

    /// Force cancel orders for a liquidatable user.
    #[wasm_bindgen(js_name = forceCancelOrders)]
    pub fn force_cancel_orders(user_address: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::ForceCancelOrders {
                user_address: parse_addr(user_address)?,
            }),
        })
    }

    /// Execute active trigger orders for a market.
    #[wasm_bindgen(js_name = executeTriggerOrders)]
    pub fn execute_trigger_orders(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::ExecuteTriggerOrders {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Apply funding to user accounts.
    #[wasm_bindgen(js_name = applyFunding)]
    pub fn apply_funding(addresses: Vec<String>) -> WasmResult<WasmCallMessage> {
        let addrs = addresses
            .iter()
            .map(|s| parse_addr(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::ApplyFunding { addresses: addrs }),
        })
    }

    /// Accrue borrow/lend interest.
    #[wasm_bindgen(js_name = accrueBorrowLendInterest)]
    pub fn accrue_borrow_lend_interest() -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::AccrueBorrowLendInterest {}),
        })
    }

    /// Execute TWAP orders for a market.
    #[wasm_bindgen(js_name = executeTwapOrders)]
    pub fn execute_twap_orders(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::ExecuteTwapOrders {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Activate TWAP orders for markets.
    #[wasm_bindgen(js_name = activateTwapOrders)]
    pub fn activate_twap_orders(market_ids: Vec<u16>) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Public(PublicAction::ActivateTwapOrders {
                market_ids: market_ids.into_iter().map(MarketId).collect(),
            }),
        })
    }

    // ═════════════════════════════════════════════════════════════════════════
    // User operations
    // ═════════════════════════════════════════════════════════════════════════

    /// Deposit assets to perp margin account.
    #[wasm_bindgen(js_name = deposit)]
    pub fn deposit(asset_id: u16, amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::Deposit {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Withdraw assets from perp margin account.
    #[wasm_bindgen(js_name = withdraw)]
    pub fn withdraw(asset_id: u16, amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::Withdraw {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Deposit assets to spot collateral.
    #[wasm_bindgen(js_name = depositSpotCollateral)]
    pub fn deposit_spot_collateral(asset_id: u16, amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DepositSpotCollateral {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Withdraw assets from spot collateral.
    #[wasm_bindgen(js_name = withdrawSpotCollateral)]
    pub fn withdraw_spot_collateral(asset_id: u16, amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::WithdrawSpotCollateral {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Transfer assets between perp margin and spot collateral.
    ///
    /// `direction`: `"margin_to_spot"` or `"spot_to_margin"`.
    #[wasm_bindgen(js_name = transferSpotCollateral)]
    pub fn transfer_spot_collateral(
        asset_id: u16,
        amount: &str,
        direction: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::TransferSpotCollateral {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
                direction: from_json(&format!("\"{direction}\""))?,
                sub_account_index,
            }),
        })
    }

    /// Borrow assets from spot pool.
    #[wasm_bindgen(js_name = borrowSpot)]
    pub fn borrow_spot(
        asset_id: u16,
        amount: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::BorrowSpot {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
                sub_account_index,
            }),
        })
    }

    /// Create a new sub-account.
    #[wasm_bindgen(js_name = createSubAccount)]
    pub fn create_sub_account(index: u8) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CreateSubAccount { index }),
        })
    }

    /// Transfer assets between main account and sub-account.
    #[wasm_bindgen(js_name = transferToSubAccount)]
    pub fn transfer_to_sub_account(
        asset_id: u16,
        amount: &str,
        sub_account_index: u8,
        to_sub_account: bool,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::TransferToSubAccount {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
                sub_account_index,
                to_sub_account,
            }),
        })
    }

    /// Delegate trading permissions to another address.
    #[wasm_bindgen(js_name = delegateUser)]
    pub fn delegate_user(delegate: &str, name: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DelegateUser {
                delegate: parse_addr(delegate)?,
                name: name.into(),
            }),
        })
    }

    /// Revoke delegation from an address.
    #[wasm_bindgen(js_name = revokeDelegation)]
    pub fn revoke_delegation(delegate: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::RevokeDelegation {
                delegate: parse_addr(delegate)?,
            }),
        })
    }

    /// Update maximum leverage for a market.
    #[wasm_bindgen(js_name = updateMaxLeverage)]
    pub fn update_max_leverage(
        market_id: u16,
        max_leverage: u16,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::UpdateMaxLeverage {
                market_id: MarketId(market_id),
                max_leverage,
                sub_account_index,
            }),
        })
    }

    /// Claim referral rewards.
    #[wasm_bindgen(js_name = claimReferralRewards)]
    pub fn claim_referral_rewards(asset_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::ClaimReferralRewards {
                asset_id: AssetId(asset_id),
            }),
        })
    }

    /// Place new orders on a market.
    ///
    /// `orders_json` is a JSON array of `NewOrderArgs`.
    #[wasm_bindgen(js_name = placeOrders)]
    pub fn place_orders(
        market_id: u16,
        orders_json: &str,
        replace: bool,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::PlaceOrders {
                market_id: MarketId(market_id),
                orders: from_json(orders_json)?,
                replace,
                sub_account_index,
            }),
        })
    }

    /// Amend existing orders (cancel + place).
    ///
    /// `orders_json` is a JSON array of `AmendOrderArgs`.
    #[wasm_bindgen(js_name = amendOrders)]
    pub fn amend_orders(
        market_id: u16,
        orders_json: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::AmendOrders {
                market_id: MarketId(market_id),
                orders: from_json(orders_json)?,
                sub_account_index,
            }),
        })
    }

    /// Cancel specific orders.
    ///
    /// `orders_json` is a JSON array of `CancelOrderArgs`.
    #[wasm_bindgen(js_name = cancelOrders)]
    pub fn cancel_orders(
        market_id: u16,
        orders_json: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelOrders {
                market_id: MarketId(market_id),
                orders: from_json(orders_json)?,
                sub_account_index,
            }),
        })
    }

    /// Cancel all orders on a market.
    #[wasm_bindgen(js_name = cancelMarketOrders)]
    pub fn cancel_market_orders(
        market_id: u16,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelMarketOrders {
                market_id: MarketId(market_id),
                sub_account_index,
            }),
        })
    }

    /// Create trigger orders.
    ///
    /// `trigger_orders_json` is a JSON array of `NewTriggerOrderArgs`.
    #[wasm_bindgen(js_name = createTriggerOrders)]
    pub fn create_trigger_orders(
        market_id: u16,
        trigger_orders_json: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CreateTriggerOrders {
                market_id: MarketId(market_id),
                trigger_orders: from_json(trigger_orders_json)?,
                sub_account_index,
            }),
        })
    }

    /// Create take-profit/stop-loss for a perp position.
    ///
    /// `tpsl_pair_json` is a JSON `TpslPair` object.
    #[wasm_bindgen(js_name = createPositionTpsl)]
    pub fn create_position_tpsl(
        market_id: u16,
        tpsl_pair_json: &str,
        size: Option<String>,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        let size = size.as_deref().map(parse_dec).transpose()?;
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CreatePositionTpsl {
                market_id: MarketId(market_id),
                tpsl_pair: from_json(tpsl_pair_json)?,
                size,
                sub_account_index,
            }),
        })
    }

    /// Cancel trigger orders.
    #[wasm_bindgen(js_name = cancelTriggerOrders)]
    pub fn cancel_trigger_orders(
        market_id: u16,
        trigger_order_ids: Vec<u64>,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelTriggerOrders {
                market_id: MarketId(market_id),
                trigger_order_ids: trigger_order_ids
                    .into_iter()
                    .map(TriggerOrderId)
                    .collect(),
                sub_account_index,
            }),
        })
    }

    /// Create a TWAP order.
    ///
    /// `twap_order_args_json` is a JSON `NewTwapOrderArgs` object.
    #[wasm_bindgen(js_name = createTwapOrder)]
    pub fn create_twap_order(
        market_id: u16,
        twap_order_args_json: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CreateTwapOrder {
                market_id: MarketId(market_id),
                twap_order_args: from_json(twap_order_args_json)?,
                sub_account_index,
            }),
        })
    }

    /// Cancel a TWAP order.
    #[wasm_bindgen(js_name = cancelTwapOrder)]
    pub fn cancel_twap_order(
        market_id: u16,
        twap_id: u64,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelTwapOrder {
                market_id: MarketId(market_id),
                twap_id: TwapId(twap_id),
                sub_account_index,
            }),
        })
    }

    /// Cancel all orders (perp and spot).
    #[wasm_bindgen(js_name = cancelAllOrders)]
    pub fn cancel_all_orders(sub_account_index: Option<u8>) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelAllOrders { sub_account_index }),
        })
    }

    /// Deposit USDC to the PnL pool.
    #[wasm_bindgen(js_name = depositToPnlPool)]
    pub fn deposit_to_pnl_pool(usdc_amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DepositToPnlPool {
                usdc_amount: parse_dec(usdc_amount)?,
            }),
        })
    }

    /// Settle user's PnL from the pool.
    #[wasm_bindgen(js_name = settleFromPnlPool)]
    pub fn settle_from_pnl_pool(sub_account_index: Option<u8>) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::SettleFromPnlPool { sub_account_index }),
        })
    }

    /// Deposit to the insurance fund.
    #[wasm_bindgen(js_name = depositToInsuranceFund)]
    pub fn deposit_to_insurance_fund(usdc_amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DepositToInsuranceFund {
                usdc_amount: parse_dec(usdc_amount)?,
            }),
        })
    }

    /// Deposit to protocol treasury.
    #[wasm_bindgen(js_name = depositToTreasury)]
    pub fn deposit_to_treasury(asset_id: u16, amount: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DepositToTreasury {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Claim accumulated borrow/lend protocol fees.
    #[wasm_bindgen(js_name = claimBorrowLendFees)]
    pub fn claim_borrow_lend_fees() -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::ClaimBorrowLendFees {}),
        })
    }

    /// Create a new vault.
    ///
    /// `args_json` is a JSON `CreateVaultArgs` object.
    #[wasm_bindgen(js_name = createVault)]
    pub fn create_vault(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CreateVault {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Deposit assets to a vault.
    #[wasm_bindgen(js_name = depositToVault)]
    pub fn deposit_to_vault(
        vault_address: &str,
        asset_id: u16,
        amount: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::DepositToVault {
                vault_address: parse_addr(vault_address)?,
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Queue a withdrawal from a vault.
    #[wasm_bindgen(js_name = queueWithdrawal)]
    pub fn queue_withdrawal(vault_address: &str, shares: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::QueueWithdrawal {
                vault_address: parse_addr(vault_address)?,
                shares: parse_dec(shares)?,
            }),
        })
    }

    /// Cancel a queued withdrawal.
    #[wasm_bindgen(js_name = cancelQueuedWithdrawal)]
    pub fn cancel_queued_withdrawal(vault_address: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::CancelQueuedWithdrawal {
                vault_address: parse_addr(vault_address)?,
            }),
        })
    }

    /// Force withdraw from a vault (bypasses queue).
    #[wasm_bindgen(js_name = forceWithdrawVault)]
    pub fn force_withdraw_vault(
        vault_address: &str,
        shares: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::ForceWithdrawVault {
                vault_address: parse_addr(vault_address)?,
                shares: parse_dec(shares)?,
            }),
        })
    }

    /// Backstop liquidation for perp positions.
    ///
    /// `positions_json` is an optional JSON array of `BackstopLiquidatePerpPositionArgs`.
    #[wasm_bindgen(js_name = backstopLiquidatePerpPositions)]
    pub fn backstop_liquidate_perp_positions(
        address: &str,
        positions_json: Option<String>,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        let positions = positions_json
            .as_deref()
            .map(from_json)
            .transpose()?;
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::BackstopLiquidatePerpPositions {
                address: parse_addr(address)?,
                positions,
                sub_account_index,
            }),
        })
    }

    /// Liquidate borrow/lend liability.
    #[wasm_bindgen(js_name = liquidateBorrowLendLiability)]
    pub fn liquidate_borrow_lend_liability(
        liquidatee_address: &str,
        liability_asset_id: u16,
        collateral_asset_id: u16,
        liability_amount: &str,
        sub_account_index: Option<u8>,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::User(UserAction::LiquidateBorrowLendLiability {
                liquidatee_address: parse_addr(liquidatee_address)?,
                liability_asset_id: AssetId(liability_asset_id),
                collateral_asset_id: AssetId(collateral_asset_id),
                liability_amount: parse_dec(liability_amount)?,
                sub_account_index,
            }),
        })
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Vault leader operations
    // ═════════════════════════════════════════════════════════════════════════

    /// Update vault configuration (leader only).
    ///
    /// `args_json` is a JSON `UpdateVaultConfigArgs` object.
    #[wasm_bindgen(js_name = updateVaultConfig)]
    pub fn update_vault_config(
        vault_address: &str,
        args_json: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::UpdateVaultConfig {
                vault_address: parse_addr(vault_address)?,
                args: from_json(args_json)?,
            }),
        })
    }

    /// Process pending vault withdrawals.
    #[wasm_bindgen(js_name = processWithdrawalQueue)]
    pub fn process_withdrawal_queue(vault_address: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::ProcessWithdrawalQueue {
                vault_address: parse_addr(vault_address)?,
            }),
        })
    }

    /// Whitelist a depositor for the vault.
    #[wasm_bindgen(js_name = whitelistDepositor)]
    pub fn whitelist_depositor(
        vault_address: &str,
        user_address: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::WhitelistDepositor {
                vault_address: parse_addr(vault_address)?,
                user_address: parse_addr(user_address)?,
            }),
        })
    }

    /// Remove a depositor from the vault whitelist.
    #[wasm_bindgen(js_name = unwhitelistDepositor)]
    pub fn unwhitelist_depositor(
        vault_address: &str,
        user_address: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::UnwhitelistDepositor {
                vault_address: parse_addr(vault_address)?,
                user_address: parse_addr(user_address)?,
            }),
        })
    }

    /// Delegate vault trading to another address.
    #[wasm_bindgen(js_name = delegateVaultUser)]
    pub fn delegate_vault_user(
        vault_address: &str,
        delegate: &str,
        name: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::DelegateVaultUser {
                vault_address: parse_addr(vault_address)?,
                delegate: parse_addr(delegate)?,
                name: name.into(),
            }),
        })
    }

    /// Revoke vault trading delegation.
    #[wasm_bindgen(js_name = revokeVaultDelegation)]
    pub fn revoke_vault_delegation(
        vault_address: &str,
        delegate: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Vault(VaultAction::RevokeVaultDelegation {
                vault_address: parse_addr(vault_address)?,
                delegate: parse_addr(delegate)?,
            }),
        })
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Keeper operations
    // ═════════════════════════════════════════════════════════════════════════

    /// Update oracle prices.
    ///
    /// `prices_json` is a JSON array of `OraclePriceUpdateArgs`.
    /// `publish_timestamp` is microseconds since Unix epoch.
    #[wasm_bindgen(js_name = updateOraclePrices)]
    pub fn update_oracle_prices(
        prices_json: &str,
        publish_timestamp: i64,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdateOraclePrices {
                prices: from_json(prices_json)?,
                publish_timestamp: UnixTimestampMicros::from_micros(publish_timestamp),
            }),
        })
    }

    /// Update mark prices.
    ///
    /// `prices_json` is a JSON array of `MarkPriceUpdateArgs`.
    /// `publish_timestamp` is microseconds since Unix epoch.
    #[wasm_bindgen(js_name = updateMarkPrices)]
    pub fn update_mark_prices(
        prices_json: &str,
        publish_timestamp: i64,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdateMarkPrices {
                prices: from_json(prices_json)?,
                publish_timestamp: UnixTimestampMicros::from_micros(publish_timestamp),
            }),
        })
    }

    /// Update premium indexes for markets.
    #[wasm_bindgen(js_name = updatePremiumIndexes)]
    pub fn update_premium_indexes(market_ids: Vec<u16>) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdatePremiumIndexes {
                market_ids: market_ids.into_iter().map(MarketId).collect(),
            }),
        })
    }

    /// Update funding rates for markets.
    #[wasm_bindgen(js_name = updateFunding)]
    pub fn update_funding(market_ids: Vec<u16>) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdateFunding {
                market_ids: market_ids.into_iter().map(MarketId).collect(),
            }),
        })
    }

    /// Add trading credits to a user.
    #[wasm_bindgen(js_name = addTradingCredits)]
    pub fn add_trading_credits(
        user_address: &str,
        amount: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::AddTradingCredits {
                user_address: parse_addr(user_address)?,
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Remove trading credits from a user.
    #[wasm_bindgen(js_name = removeTradingCredits)]
    pub fn remove_trading_credits(
        user_address: &str,
        amount: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::RemoveTradingCredits {
                user_address: parse_addr(user_address)?,
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Update user's fee tier.
    ///
    /// `fee_tier`: one of `"Tier0"` .. `"Tier4"`.
    #[wasm_bindgen(js_name = updateUserFeeTier)]
    pub fn update_user_fee_tier(address: &str, fee_tier: &str) -> WasmResult<WasmCallMessage> {
        let tier: FeeTier = from_json(&format!("\"{fee_tier}\""))?;
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdateUserFeeTier {
                address: parse_addr(address)?,
                fee_tier: tier,
            }),
        })
    }

    /// Update a user's fee discount (in bps).
    #[wasm_bindgen(js_name = updateUserFeeDiscountBps)]
    pub fn update_user_fee_discount_bps(
        address: &str,
        fee_discount_bps: u16,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::UpdateUserFeeDiscountBps {
                address: parse_addr(address)?,
                fee_discount_bps,
            }),
        })
    }

    /// Set a user's cumulative referral rewards to an absolute amount.
    #[wasm_bindgen(js_name = setCumulativeReferralRewards)]
    pub fn set_cumulative_referral_rewards(
        address: &str,
        asset_id: u16,
        amount: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Keeper(KeeperAction::SetCumulativeReferralRewards {
                address: parse_addr(address)?,
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Admin operations
    // ═════════════════════════════════════════════════════════════════════════

    /// Initialize a new perp market.
    ///
    /// `args_json` is a JSON `InitPerpMarketArgs` object.
    #[wasm_bindgen(js_name = initPerpMarket)]
    pub fn init_perp_market(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::InitPerpMarket {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update perp market configuration.
    ///
    /// `args_json` is a JSON `UpdatePerpMarketArgs` object.
    #[wasm_bindgen(js_name = updatePerpMarket)]
    pub fn update_perp_market(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdatePerpMarket {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Initialize a new spot market.
    ///
    /// `args_json` is a JSON `InitSpotMarketArgs` object.
    #[wasm_bindgen(js_name = initSpotMarket)]
    pub fn init_spot_market(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::InitSpotMarket {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update spot market configuration.
    ///
    /// `args_json` is a JSON `UpdateSpotMarketArgs` object.
    #[wasm_bindgen(js_name = updateSpotMarket)]
    pub fn update_spot_market(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateSpotMarket {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Halt a perp market with a settlement price.
    #[wasm_bindgen(js_name = haltPerpMarket)]
    pub fn halt_perp_market(
        market_id: u16,
        settlement_price: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::HaltPerpMarket {
                market_id: MarketId(market_id),
                settlement_price: parse_dec(settlement_price)?,
            }),
        })
    }

    /// Unhalt a perp market.
    #[wasm_bindgen(js_name = unhaltPerpMarket)]
    pub fn unhalt_perp_market(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UnhaltPerpMarket {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Halt a spot market.
    #[wasm_bindgen(js_name = haltSpotMarket)]
    pub fn halt_spot_market(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::HaltSpotMarket {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Unhalt a spot market.
    #[wasm_bindgen(js_name = unhaltSpotMarket)]
    pub fn unhalt_spot_market(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UnhaltSpotMarket {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Prune market data.
    #[wasm_bindgen(js_name = pruneMarket)]
    pub fn prune_market(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::PruneMarket {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Delete a market.
    #[wasm_bindgen(js_name = deleteMarket)]
    pub fn delete_market(market_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::DeleteMarket {
                market_id: MarketId(market_id),
            }),
        })
    }

    /// Cleanup user market state.
    #[wasm_bindgen(js_name = cleanupUserMarketState)]
    pub fn cleanup_user_market_state(
        market_id: u16,
        users: Vec<String>,
    ) -> WasmResult<WasmCallMessage> {
        let addrs = users
            .iter()
            .map(|s| parse_addr(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::CleanupUserMarketState {
                market_id: MarketId(market_id),
                users: addrs,
            }),
        })
    }

    /// Update perp market leverage table.
    ///
    /// `args_json` is a JSON `SurrogateLeverageTableArgs` object.
    #[wasm_bindgen(js_name = updatePerpLeverageTable)]
    pub fn update_perp_leverage_table(
        market_id: u16,
        args_json: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdatePerpLeverageTable {
                market_id: MarketId(market_id),
                args: from_json(args_json)?,
            }),
        })
    }

    /// Delete an asset.
    #[wasm_bindgen(js_name = deleteAsset)]
    pub fn delete_asset(asset_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::DeleteAsset {
                asset_id: AssetId(asset_id),
            }),
        })
    }

    /// Initialize asset info.
    ///
    /// `args_json` is a JSON `InitAssetInfoArgs` object.
    #[wasm_bindgen(js_name = initAssetInfo)]
    pub fn init_asset_info(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::InitAssetInfo {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update asset info.
    ///
    /// `args_json` is a JSON `UpdateAssetInfoArgs` object.
    #[wasm_bindgen(js_name = updateAssetInfo)]
    pub fn update_asset_info(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateAssetInfo {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Initialize a borrow/lend pool.
    ///
    /// `args_json` is a JSON `InitBorrowLendPoolArgs` object.
    #[wasm_bindgen(js_name = initBorrowLendPool)]
    pub fn init_borrow_lend_pool(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::InitBorrowLendPool {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update borrow/lend pool configuration.
    ///
    /// `args_json` is a JSON `UpdateBorrowLendPoolArgs` object.
    #[wasm_bindgen(js_name = updateBorrowLendPool)]
    pub fn update_borrow_lend_pool(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateBorrowLendPool {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Halt a borrow/lend pool.
    #[wasm_bindgen(js_name = haltBorrowLendPool)]
    pub fn halt_borrow_lend_pool(asset_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::HaltBorrowLendPool {
                asset_id: AssetId(asset_id),
            }),
        })
    }

    /// Unhalt a borrow/lend pool.
    #[wasm_bindgen(js_name = unhaltBorrowLendPool)]
    pub fn unhalt_borrow_lend_pool(asset_id: u16) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UnhaltBorrowLendPool {
                asset_id: AssetId(asset_id),
            }),
        })
    }

    /// Update global configuration.
    ///
    /// `args_json` is a JSON `UpdateGlobalConfigArgs` object.
    #[wasm_bindgen(js_name = updateGlobalConfig)]
    pub fn update_global_config(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateGlobalConfig {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update perp liquidation configuration.
    ///
    /// `args_json` is a JSON `UpdatePerpLiquidationConfigArgs` object.
    #[wasm_bindgen(js_name = updatePerpLiquidationConfig)]
    pub fn update_perp_liquidation_config(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdatePerpLiquidationConfig {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update global vault configuration.
    ///
    /// `args_json` is a JSON `UpdateGlobalVaultConfigArgs` object.
    #[wasm_bindgen(js_name = updateGlobalVaultConfig)]
    pub fn update_global_vault_config(args_json: &str) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateGlobalVaultConfig {
                args: from_json(args_json)?,
            }),
        })
    }

    /// Update admin addresses.
    ///
    /// `admin_type`: one of `"Protocol"`, `"Funding"`, `"Pricing"`,
    ///               `"FeeTier"`, `"Credits"`, `"Referrals"`.
    #[wasm_bindgen(js_name = updateAdmin)]
    pub fn update_admin(admin_type: &str, new_admin: &str) -> WasmResult<WasmCallMessage> {
        let at: AdminType = from_json(&format!("\"{admin_type}\""))?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::UpdateAdmin {
                admin_type: at,
                new_admin: parse_addr(new_admin)?,
            }),
        })
    }

    /// Withdraw from protocol treasury.
    #[wasm_bindgen(js_name = withdrawFromTreasury)]
    pub fn withdraw_from_treasury(
        asset_id: u16,
        amount: &str,
        to: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::WithdrawFromTreasury {
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
                to: parse_addr(to)?,
            }),
        })
    }

    /// Force cancel user orders (admin).
    ///
    /// `cancels_json` is a JSON array of `[market_id, order_id, address]` tuples.
    #[wasm_bindgen(js_name = adminCancelOrders)]
    pub fn admin_cancel_orders(cancels_json: &str) -> WasmResult<WasmCallMessage> {
        let raw: Vec<(u16, u64, String)> = from_json(cancels_json)?;
        let cancels = raw
            .into_iter()
            .map(|(m, o, a)| Ok((MarketId(m), OrderId(o), parse_addr(&a)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::CancelOrders { cancels }),
        })
    }

    /// Force cancel user trigger orders (admin).
    ///
    /// `cancels_json` is a JSON array of `[market_id, trigger_order_id, address]` tuples.
    #[wasm_bindgen(js_name = adminCancelTriggerOrders)]
    pub fn admin_cancel_trigger_orders(cancels_json: &str) -> WasmResult<WasmCallMessage> {
        let raw: Vec<(u16, u64, String)> = from_json(cancels_json)?;
        let cancels = raw
            .into_iter()
            .map(|(m, t, a)| Ok((MarketId(m), TriggerOrderId(t), parse_addr(&a)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::CancelTriggerOrders { cancels }),
        })
    }

    /// Force settle perp positions.
    #[wasm_bindgen(js_name = forceSettlePerpPosition)]
    pub fn force_settle_perp_position(
        market_id: u16,
        users: Vec<String>,
    ) -> WasmResult<WasmCallMessage> {
        let addrs = users
            .iter()
            .map(|s| parse_addr(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::ForceSettlePerpPosition {
                market_id: MarketId(market_id),
                users: addrs,
            }),
        })
    }

    /// Auto-deleverage positions.
    #[wasm_bindgen(js_name = autoDeleverage)]
    pub fn auto_deleverage(
        counterparty_a: &str,
        counterparty_a_sub_account_index: Option<u8>,
        counterparty_b: &str,
        counterparty_b_sub_account_index: Option<u8>,
        market_id: u16,
        size: Option<String>,
        settlement_price: &str,
    ) -> WasmResult<WasmCallMessage> {
        let size = size.as_deref().map(parse_dec).transpose()?;
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::AutoDeleverage {
                counterparty_a: parse_addr(counterparty_a)?,
                counterparty_a_sub_account_index,
                counterparty_b: parse_addr(counterparty_b)?,
                counterparty_b_sub_account_index,
                market_id: MarketId(market_id),
                size,
                settlement_price: parse_dec(settlement_price)?,
            }),
        })
    }

    /// Admin deposit to any user account.
    #[wasm_bindgen(js_name = adminDeposit)]
    pub fn admin_deposit(
        user_address: &str,
        asset_id: u16,
        amount: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::Deposit {
                user_address: parse_addr(user_address)?,
                asset_id: AssetId(asset_id),
                amount: parse_dec(amount)?,
            }),
        })
    }

    /// Force remove a delegate from a delegator.
    #[wasm_bindgen(js_name = forceRemoveDelegate)]
    pub fn force_remove_delegate(
        delegator: &str,
        delegate: &str,
    ) -> WasmResult<WasmCallMessage> {
        Ok(WasmCallMessage {
            inner: CallMessage::Admin(AdminAction::ForceRemoveDelegate {
                delegator: parse_addr(delegator)?,
                delegate: parse_addr(delegate)?,
            }),
        })
    }
}

// ── WasmTransaction ───────────────────────────────────────────────────────────

/// An opaque handle to a signed `Transaction`.
///
/// Passed directly to `Client.submitTransaction` or serialised to base64 via
/// `toBase64()` for WebSocket submission — no redundant encode/decode at the
/// JS boundary.
#[wasm_bindgen(js_name = Transaction)]
pub struct WasmTransaction {
    pub(crate) inner: Transaction,
}

#[wasm_bindgen(js_class = Transaction)]
impl WasmTransaction {
    /// Borsh-serialise and base64-encode the transaction.
    ///
    /// Use this when you need to pass the transaction over a WebSocket
    /// connection (e.g. `WebsocketHandle.orderPlace`).
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        use bullet_rust_sdk::Client;
        Ok(Client::sign_to_base64(&self.inner)?)
    }
}

// ── Client methods ────────────────────────────────────────────────────────────

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Build and sign a transaction, returning an opaque `Transaction` handle.
    ///
    /// - `call_msg` – a `CallMessage` constructed via a factory method
    /// - `max_fee`  – maximum fee in base units
    /// - `keypair`  – signing keypair
    #[wasm_bindgen(js_name = buildSignedTransaction)]
    pub fn build_signed_transaction(
        &self,
        call_msg: WasmCallMessage,
        max_fee: u64,
        keypair: &WasmKeypair,
    ) -> WasmResult<WasmTransaction> {
        let unsigned = self
            .inner
            .build_transaction(call_msg.inner, u128::from(max_fee))?;
        let signed = self.inner.sign_transaction(unsigned, &keypair.inner)?;
        Ok(WasmTransaction { inner: signed })
    }

    /// Submit a signed transaction via REST.
    ///
    /// Returns a JSON string of the `SubmitTxResponse`.
    #[wasm_bindgen(js_name = submitTransaction)]
    pub async fn submit_transaction(&self, tx: &WasmTransaction) -> WasmResult<String> {
        let resp = self.inner.submit_transaction(&tx.inner).await?;
        Ok(serde_json::to_string(&resp)?)
    }
}
