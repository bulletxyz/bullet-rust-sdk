//! Ergonomic order builder for placing orders on Bullet.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::{Client, NewOrder};
//! use bullet_rust_sdk::types::{Side, OrderType};
//!
//! # async fn example(client: Client) -> Result<(), Box<dyn std::error::Error>> {
//! // Single order — resolves "BTC-USD" → MarketId automatically
//! NewOrder::builder("BTC-USD", Side::Bid, OrderType::Limit)
//!     .price("50000")?
//!     .size("0.1")?
//!     .reduce_only(true)
//!     .submit(&client)
//!     .await?;
//!
//! // Batch — all orders on the same market in one transaction
//! NewOrder::submit_batch(
//!     "BTC-USD",
//!     vec![
//!         NewOrder::args(Side::Bid, OrderType::Limit)
//!             .price("50000")?.size("0.1")?.build()?,
//!         NewOrder::args(Side::Ask, OrderType::ImmediateOrCancel)
//!             .price("51000")?.size("0.1")?.build()?,
//!     ],
//!     &client,
//! ).await?;
//! # Ok(())
//! # }
//! ```

use std::str::FromStr;

use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::NewOrderArgs;
use bullet_exchange_interface::types::{MarketId, OrderType, Side};

use crate::generated::types::SubmitTxResponse;
use crate::types::CallMessage;
use crate::{Client, SDKError, SDKResult};

/// A fully-constructed order, ready to be submitted.
///
/// Returned by [`NewOrderBuilder::build`] for use with [`NewOrder::submit_batch`].
pub struct NewOrder {
    /// The symbol string used to resolve `MarketId` at submit time.
    pub(crate) symbol: String,
    pub(crate) args: NewOrderArgs,
}

/// Builder for a single order. Construct via [`NewOrder::builder`].
pub struct NewOrderBuilder {
    symbol: String,
    side: Side,
    order_type: OrderType,
    price: Option<PositiveDecimal>,
    size: Option<PositiveDecimal>,
    reduce_only: bool,
    cloid: Option<u64>,
}

/// Argument-only builder (no symbol) for use with [`NewOrder::submit_batch`].
/// Construct via [`NewOrder::args`].
pub struct NewOrderArgsBuilder {
    side: Side,
    order_type: OrderType,
    price: Option<PositiveDecimal>,
    size: Option<PositiveDecimal>,
    reduce_only: bool,
    cloid: Option<u64>,
}

impl NewOrder {
    /// Start building a single order for the given symbol.
    ///
    /// `symbol` (e.g. `"BTC-USD"`) is resolved to a `MarketId` at submit time
    /// via the exchange info endpoint.
    pub fn builder(
        symbol: impl Into<String>,
        side: Side,
        order_type: OrderType,
    ) -> NewOrderBuilder {
        NewOrderBuilder {
            symbol: symbol.into(),
            side,
            order_type,
            price: None,
            size: None,
            reduce_only: false,
            cloid: None,
        }
    }

    /// Start building order args without a symbol, for use with [`submit_batch`](Self::submit_batch).
    pub fn args(side: Side, order_type: OrderType) -> NewOrderArgsBuilder {
        NewOrderArgsBuilder {
            side,
            order_type,
            price: None,
            size: None,
            reduce_only: false,
            cloid: None,
        }
    }

    /// Submit multiple orders on the same market in a single transaction.
    ///
    /// `symbol` is resolved to a `MarketId` via the exchange info endpoint.
    ///
    /// # Example
    ///
    /// ```ignore
    /// NewOrder::submit_batch(
    ///     "BTC-USD",
    ///     vec![
    ///         NewOrder::args(Side::Bid, OrderType::Limit).price("50000")?.size("0.1")?.build()?,
    ///         NewOrder::args(Side::Ask, OrderType::ImmediateOrCancel).price("51000")?.size("0.1")?.build()?,
    ///     ],
    ///     &client,
    /// ).await?;
    /// ```
    pub async fn submit_batch(
        symbol: &str,
        orders: Vec<NewOrderArgs>,
        client: &Client,
    ) -> SDKResult<SubmitTxResponse> {
        let market_id = client.market_id_for(symbol).await?;
        let call_msg = place_orders_msg(market_id, orders, false);
        crate::Transaction::builder()
            .call_message(call_msg)
            .send(client)
            .await
    }
}

impl NewOrderBuilder {
    /// Set the limit price. Required for `Limit`, `PostOnly`, `FillOrKill`, and `ImmediateOrCancel` orders.
    pub fn price(mut self, price: &str) -> SDKResult<Self> {
        self.price = Some(parse_decimal(price)?);
        Ok(self)
    }

    /// Set the order size.
    pub fn size(mut self, size: &str) -> SDKResult<Self> {
        self.size = Some(parse_decimal(size)?);
        Ok(self)
    }

    /// Mark this as a reduce-only order. Default: `false`.
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// Set an optional client order ID.
    pub fn cloid(mut self, id: u64) -> Self {
        self.cloid = Some(id);
        self
    }

    /// Build the order args without yet resolving the symbol.
    ///
    /// Use this when you want to inspect the args before submitting,
    /// or when constructing args for `submit_batch` with an explicit symbol.
    pub fn build(self) -> SDKResult<NewOrder> {
        let args = build_args(
            self.side,
            self.order_type,
            self.price,
            self.size,
            self.reduce_only,
            self.cloid,
        )?;
        Ok(NewOrder {
            symbol: self.symbol,
            args,
        })
    }

    /// Resolve the symbol, build, sign, and submit the order.
    ///
    /// This is the primary ergonomic path for placing a single order.
    pub async fn submit(self, client: &Client) -> SDKResult<SubmitTxResponse> {
        let order = self.build()?;
        let market_id = client.market_id_for(&order.symbol).await?;
        let call_msg = place_orders_msg(market_id, vec![order.args], false);
        crate::Transaction::builder()
            .call_message(call_msg)
            .send(client)
            .await
    }
}

impl NewOrderArgsBuilder {
    /// Set the limit price.
    pub fn price(mut self, price: &str) -> SDKResult<Self> {
        self.price = Some(parse_decimal(price)?);
        Ok(self)
    }

    /// Set the order size.
    pub fn size(mut self, size: &str) -> SDKResult<Self> {
        self.size = Some(parse_decimal(size)?);
        Ok(self)
    }

    /// Mark as reduce-only. Default: `false`.
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// Set an optional client order ID.
    pub fn cloid(mut self, id: u64) -> Self {
        self.cloid = Some(id);
        self
    }

    /// Build the `NewOrderArgs`.
    pub fn build(self) -> SDKResult<NewOrderArgs> {
        build_args(
            self.side,
            self.order_type,
            self.price,
            self.size,
            self.reduce_only,
            self.cloid,
        )
    }
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn parse_decimal(s: &str) -> SDKResult<PositiveDecimal> {
    PositiveDecimal::from_str(s)
        .map_err(|e| SDKError::InvalidPrivateKey(format!("Invalid decimal '{s}': {e:?}")))
}

fn build_args(
    side: Side,
    order_type: OrderType,
    price: Option<PositiveDecimal>,
    size: Option<PositiveDecimal>,
    reduce_only: bool,
    cloid: Option<u64>,
) -> SDKResult<NewOrderArgs> {
    let price = price.ok_or_else(|| SDKError::SerializationError("price is required".into()))?;
    let size = size.ok_or_else(|| SDKError::SerializationError("size is required".into()))?;
    Ok(NewOrderArgs {
        price,
        size,
        side,
        order_type,
        reduce_only,
        client_order_id: cloid.map(|id| bullet_exchange_interface::types::ClientOrderId(id)),
        pending_tpsl_pair: None,
    })
}

fn place_orders_msg(
    market_id: MarketId,
    orders: Vec<NewOrderArgs>,
    replace: bool,
) -> CallMessage {
    CallMessage::User(bullet_exchange_interface::message::UserAction::PlaceOrders {
        market_id,
        orders,
        replace,
        sub_account_index: None,
    })
}
