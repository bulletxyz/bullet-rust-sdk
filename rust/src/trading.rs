//! High-level convenience methods for common trading operations.
//!
//! These methods handle `CallMessage` construction, transaction signing, and
//! submission internally — reducing a typical order flow from ~15 lines to ~5.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::*;
//!
//! let client = Client::builder()
//!     .network(Network::Mainnet)
//!     .keypair(keypair)
//!     .build()
//!     .await?;
//!
//! // Place a limit buy
//! let market_id = client.market_id("BTC-USD").unwrap();
//! let resp = client.place_orders(
//!     market_id,
//!     vec![NewOrderArgs::limit(price, size, Side::Bid)],
//!     false,
//!     None,
//! ).await?;
//! ```

use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::{AmendOrderArgs, CancelOrderArgs, NewOrderArgs};
use bullet_exchange_interface::types::{MarketId, OrderType, Side};

use crate::generated::types::SubmitTxResponse;
use crate::types::{CallMessage, UserAction};
use crate::{Client, SDKError, SDKResult, Transaction};

// ── Order construction helpers ──────────────────────────────────────────────

/// Extension constructors for [`NewOrderArgs`].
///
/// Removes the 4-field boilerplate from simple orders. For advanced fields
/// (`reduce_only`, `client_order_id`, `pending_tpsl_pair`), construct
/// `NewOrderArgs` directly.
///
/// ```ignore
/// use bullet_rust_sdk::*;
///
/// let order = NewOrderArgs::limit(price, size, Side::Bid);
/// client.place_orders(market_id, vec![order], false, None).await?;
/// ```
pub trait NewOrderExt {
    /// Create a limit order. Defaults: `reduce_only: false`, no client order ID, no TP/SL.
    fn limit(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self;
    /// Create a post-only (maker) order. Rejected if it would cross the book.
    fn post_only(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self;
    /// Create an immediate-or-cancel order (market-order equivalent).
    ///
    /// Fills what it can at the given price, cancels the rest.
    fn ioc(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self;
}

impl NewOrderExt for NewOrderArgs {
    fn limit(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self {
        new_order(price, size, side, OrderType::Limit)
    }

    fn post_only(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self {
        new_order(price, size, side, OrderType::PostOnly)
    }

    fn ioc(price: PositiveDecimal, size: PositiveDecimal, side: Side) -> Self {
        new_order(price, size, side, OrderType::ImmediateOrCancel)
    }
}

fn new_order(
    price: PositiveDecimal,
    size: PositiveDecimal,
    side: Side,
    order_type: OrderType,
) -> NewOrderArgs {
    NewOrderArgs {
        price,
        size,
        side,
        order_type,
        reduce_only: false,
        client_order_id: None,
        pending_tpsl_pair: None,
    }
}

impl Client {
    /// Place orders on a market. Signs and submits the transaction.
    ///
    /// # Arguments
    ///
    /// * `market_id` — Numeric market ID (resolve via `client.market_id("BTC-USD")`)
    /// * `orders` — One or more orders to place
    /// * `replace` — If `true`, cancel existing orders before placing new ones
    /// * `sub_account_index` — `None` for the main account, `Some(n)` for a sub-account
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bullet_rust_sdk::*;
    ///
    /// let market_id = client.market_id("BTC-USD").unwrap();
    /// let price = PositiveDecimal::try_from(rust_decimal::Decimal::from(50000))?;
    /// let size = PositiveDecimal::try_from(rust_decimal::Decimal::new(1, 3))?;
    /// let resp = client.place_orders(
    ///     market_id,
    ///     vec![NewOrderArgs::limit(price, size, Side::Bid)],
    ///     false,
    ///     None,
    /// ).await?;
    /// println!("TX: {}, status: {:?}", resp.id, resp.status);
    /// ```
    pub async fn place_orders(
        &self,
        market_id: MarketId,
        orders: Vec<NewOrderArgs>,
        replace: bool,
        sub_account_index: Option<u8>,
    ) -> SDKResult<SubmitTxResponse> {
        let call_msg = CallMessage::User(UserAction::PlaceOrders {
            market_id,
            orders,
            replace,
            sub_account_index,
        });
        let signed = Transaction::builder()
            .call_message(call_msg)
            .client(self)
            .build()?;
        self.send_transaction(&signed).await
    }

    /// Cancel specific orders on a market. Signs and submits the transaction.
    ///
    /// Cancel by exchange-assigned `OrderId`, client-assigned `ClientOrderId`, or both.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bullet_rust_sdk::*;
    ///
    /// let resp = client.cancel_orders(
    ///     MarketId(0),
    ///     vec![CancelOrderArgs {
    ///         order_id: Some(OrderId(12345)),
    ///         client_order_id: None,
    ///     }],
    ///     None,
    /// ).await?;
    /// ```
    pub async fn cancel_orders(
        &self,
        market_id: MarketId,
        orders: Vec<CancelOrderArgs>,
        sub_account_index: Option<u8>,
    ) -> SDKResult<SubmitTxResponse> {
        let call_msg = CallMessage::User(UserAction::CancelOrders {
            market_id,
            orders,
            sub_account_index,
        });
        let signed = Transaction::builder()
            .call_message(call_msg)
            .client(self)
            .build()?;
        self.send_transaction(&signed).await
    }

    /// Cancel all orders on a specific market. Signs and submits the transaction.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let resp = client.cancel_market_orders(MarketId(0), None).await?;
    /// ```
    pub async fn cancel_market_orders(
        &self,
        market_id: MarketId,
        sub_account_index: Option<u8>,
    ) -> SDKResult<SubmitTxResponse> {
        let call_msg = CallMessage::User(UserAction::CancelMarketOrders {
            market_id,
            sub_account_index,
        });
        let signed = Transaction::builder()
            .call_message(call_msg)
            .client(self)
            .build()?;
        self.send_transaction(&signed).await
    }

    /// Cancel all orders across all markets. Signs and submits the transaction.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let resp = client.cancel_all_orders(None).await?;
    /// ```
    pub async fn cancel_all_orders(
        &self,
        sub_account_index: Option<u8>,
    ) -> SDKResult<SubmitTxResponse> {
        let call_msg = CallMessage::User(UserAction::CancelAllOrders { sub_account_index });
        let signed = Transaction::builder()
            .call_message(call_msg)
            .client(self)
            .build()?;
        self.send_transaction(&signed).await
    }

    // ── Account query convenience methods ─────────────────────────────────
    //
    // These derive the account address from the client's keypair so you
    // don't have to format it manually on every call.

    /// Get the base58 address derived from the client's keypair.
    ///
    /// Returns `Err(SDKError::MissingKeypair)` if no keypair is configured.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let address = client.address()?;
    /// println!("My address: {address}"); // e.g. "5Hq3...xyz"
    /// ```
    pub fn address(&self) -> SDKResult<String> {
        let kp = self.keypair().ok_or(SDKError::MissingKeypair)?;
        Ok(kp.address())
    }

    /// Query open orders for the client's own account on a symbol.
    ///
    /// Convenience wrapper around `query_open_orders` that derives the
    /// address from the client's keypair.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let orders = client.my_open_orders("BTC-USD").await?;
    /// for o in &orders {
    ///     println!("{}: {} {} @ {}", o.order_id, o.side, o.orig_qty, o.price);
    /// }
    /// ```
    pub async fn my_open_orders(
        &self,
        symbol: &str,
    ) -> SDKResult<Vec<crate::generated::types::BinanceOrder>> {
        let address = self.address()?;
        let resp = self.query_open_orders(&address, symbol).await?;
        Ok(resp.into_inner())
    }

    /// Query account info (positions, margins) for the client's own account.
    ///
    /// Convenience wrapper around `account_info` that derives the address
    /// from the client's keypair and unwraps the response.
    pub async fn my_account(&self) -> SDKResult<crate::generated::types::Account> {
        let address = self.address()?;
        let resp = self.account_info(&address).await?;
        Ok(resp.into_inner())
    }

    /// Query balances for the client's own account.
    ///
    /// Convenience wrapper around `account_balance` that derives the address
    /// from the client's keypair and unwraps the response.
    pub async fn my_balances(&self) -> SDKResult<Vec<crate::generated::types::Balance>> {
        let address = self.address()?;
        let resp = self.account_balance(&address).await?;
        Ok(resp.into_inner())
    }

    // ── Order management ────────────────────────────────────────────────

    /// Amend (cancel + replace) existing orders. Signs and submits the transaction.
    ///
    /// Each [`AmendOrderArgs`] pairs a [`CancelOrderArgs`] with a [`NewOrderArgs`],
    /// atomically replacing the cancelled order with a new one.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bullet_rust_sdk::*;
    ///
    /// let resp = client.amend_orders(
    ///     market_id,
    ///     vec![AmendOrderArgs {
    ///         cancel: CancelOrderArgs {
    ///             order_id: Some(OrderId(12345)),
    ///             client_order_id: None,
    ///         },
    ///         place: NewOrderArgs::limit(new_price, new_size, Side::Bid),
    ///     }],
    ///     None,
    /// ).await?;
    /// ```
    pub async fn amend_orders(
        &self,
        market_id: MarketId,
        orders: Vec<AmendOrderArgs>,
        sub_account_index: Option<u8>,
    ) -> SDKResult<SubmitTxResponse> {
        let call_msg = CallMessage::User(UserAction::AmendOrders {
            market_id,
            orders,
            sub_account_index,
        });
        let signed = Transaction::builder()
            .call_message(call_msg)
            .client(self)
            .build()?;
        self.send_transaction(&signed).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn dec(s: &str) -> PositiveDecimal {
        PositiveDecimal::try_from(Decimal::from_str(s).unwrap()).unwrap()
    }

    #[test]
    fn limit_order_defaults() {
        let order = NewOrderArgs::limit(dec("50000"), dec("0.1"), Side::Bid);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.side, Side::Bid);
        assert!(!order.reduce_only);
        assert!(order.client_order_id.is_none());
        assert!(order.pending_tpsl_pair.is_none());
    }

    #[test]
    fn post_only_order_defaults() {
        let order = NewOrderArgs::post_only(dec("50000"), dec("0.1"), Side::Ask);
        assert_eq!(order.order_type, OrderType::PostOnly);
        assert_eq!(order.side, Side::Ask);
        assert!(!order.reduce_only);
        assert!(order.client_order_id.is_none());
    }

    #[test]
    fn ioc_order_defaults() {
        let order = NewOrderArgs::ioc(dec("50000"), dec("0.1"), Side::Bid);
        assert_eq!(order.order_type, OrderType::ImmediateOrCancel);
        assert!(!order.reduce_only);
        assert!(order.client_order_id.is_none());
    }
}
