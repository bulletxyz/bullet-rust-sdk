//! WebSocket subscription example.
//!
//! Connects to the trading API WebSocket and subscribes to market data.
//!
//! # Usage
//!
//! ```bash
//! # Start the trading-api server first, then:
//! cargo run -p trading-sdk --example websocket
//!
//! # Or with a custom endpoint:
//! API_ENDPOINT=http://localhost:3000 cargo run -p trading-sdk --example websocket
//! ```

#[allow(unused_imports)]
use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::NewOrderArgs;
use bullet_exchange_interface::types::Side;
use bullet_rust_sdk::{Keypair, KlineInterval, OrderbookDepth, Topic, TradingApi};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let api_endpoint =
        std::env::var("API_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    println!("Connecting to {api_endpoint}...");

    let client = TradingApi::new(&api_endpoint, None).await?;

    let mut ws = client.connect_ws().await?;
    println!("Connected to WebSocket");

    // Subscribe to multiple topics
    ws.subscribe(
        [
            Topic::agg_trade("BTC-USD"),
            Topic::depth("ETH-USD", OrderbookDepth::D10),
            Topic::book_ticker("SOL-USD"),
            Topic::kline("BTC-USD", KlineInterval::H1),
            Topic::all_tickers(),
        ],
        None,
    )
    .await?;
    println!("Subscribed to topics");

    // Receive messages
    println!("\nReceiving messages (Ctrl+C to stop):\n");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    let ask_tx = client.build_transaction(
        bullet_exchange_interface::message::CallMessage::User(
            bullet_exchange_interface::message::UserAction::PlaceOrders {
                market_id: bullet_exchange_interface::types::MarketId(0),
                orders: vec![NewOrderArgs {
                    price: 120_u8.into(),
                    size: 1_u8.into(),
                    side: Side::Ask,
                    order_type: bullet_exchange_interface::types::OrderType::ImmediateOrCancel,
                    reduce_only: false,
                    client_order_id: None,
                    pending_tpsl_pair: None,
                }],
                replace: false,
                sub_account_index: None,
            },
        ),
        10_000_000,
    )?;

    // TODO: use ENV var or generate.
    let keypair = Keypair::generate();

    #[allow(unused_mut)]
    let mut req_id = None;
    loop {
        println!("Type bid or ask to send the corresponding place order tx");
        tokio::select! {
            msg = ws.recv() => {
                match msg {
                    // TODO: Do something with the req_id
                    Ok(msg) => {
                        println!("{msg:?}\n");
                    },
                    Err(e) => {
                        eprintln!("Error: {e}");
                        break;
                    }
                }

            }
            Ok(_x) = reader.read_line(&mut line) => {
                println!("Got input");
                match line.trim() {
                    "bid" => {
                        let signed_tx = client.sign_transaction(ask_tx.clone(), &keypair)?;
                        ws.order_place(TradingApi::sign_to_base64(&signed_tx)?, req_id).await?;
                        println!("Sent bid. Got ReqId {req_id:?}");
                    },

                    "ask" => {
                        let signed_tx = client.sign_transaction(ask_tx.clone(), &keypair)?;
                        ws.order_place(TradingApi::sign_to_base64(&signed_tx)?, req_id).await?;
                        println!("Sent ask. Got ReqId {req_id:?}");
                    }
                    x => {
                        println!("Got unknown input {x}");
                        continue;
                    }
                }
            }
        }
    }

    Ok(())
}
