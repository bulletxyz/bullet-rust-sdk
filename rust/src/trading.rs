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
//! let resp = client.place_orders(
//!     MarketId(0),
//!     vec![NewOrderArgs {
//!         price: PositiveDecimal::try_from(price)?,
//!         size: PositiveDecimal::try_from(qty)?,
//!         side: Side::Bid,
//!         order_type: OrderType::PostOnly,
//!         reduce_only: false,
//!         client_order_id: None,
//!         pending_tpsl_pair: None,
//!     }],
//!     false,
//!     None,
//! ).await?;
//! ```

use bullet_exchange_interface::message::{AmendOrderArgs, CancelOrderArgs, NewOrderArgs};
use bullet_exchange_interface::types::MarketId;

use crate::generated::types::SubmitTxResponse;
use crate::types::{CallMessage, UserAction};
use crate::{Client, SDKResult, Transaction};

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
    /// let resp = client.place_orders(
    ///     MarketId(0),
    ///     vec![NewOrderArgs {
    ///         price: PositiveDecimal::try_from(rust_decimal::Decimal::from(50000))?,
    ///         size: PositiveDecimal::try_from(rust_decimal::Decimal::new(1, 3))?,
    ///         side: Side::Bid,
    ///         order_type: OrderType::Limit,
    ///         reduce_only: false,
    ///         client_order_id: None,
    ///         pending_tpsl_pair: None,
    ///     }],
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
    ///     MarketId(0),
    ///     vec![AmendOrderArgs {
    ///         cancel: CancelOrderArgs {
    ///             order_id: Some(OrderId(12345)),
    ///             client_order_id: None,
    ///         },
    ///         place: NewOrderArgs {
    ///             price: PositiveDecimal::try_from(rust_decimal::Decimal::from(51000))?,
    ///             size: PositiveDecimal::try_from(rust_decimal::Decimal::new(1, 3))?,
    ///             side: Side::Bid,
    ///             order_type: OrderType::Limit,
    ///             reduce_only: false,
    ///             client_order_id: None,
    ///             pending_tpsl_pair: None,
    ///         },
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
