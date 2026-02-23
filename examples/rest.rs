//! REST API example.
//!
//! Demonstrates querying market data and account information via REST.
//!
//! # Usage
//!
//! ```bash
//! # Start the trading-api server first, then:
//! cargo run -p trading-sdk --example rest
//!
//! # Or with a custom endpoint:
//! API_ENDPOINT=http://localhost:3000 cargo run -p trading-sdk --example rest
//!
//! # With an address for account queries:
//! ADDRESS=0x1234... cargo run -p trading-sdk --example rest
//! ```

use bullet_rust_sdk::TradingApi;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let api_endpoint =
        std::env::var("API_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    println!("Connecting to {api_endpoint}...\n");
    let api = TradingApi::new(&api_endpoint, None).await?;

    // Server health check
    println!("=== Health Check ===");
    let time = api.time().await?;
    println!("Server time: {}\n", time.server_time);

    // Exchange info
    println!("=== Exchange Info ===");
    let info = api.exchange_info().await?.into_inner();
    println!("Available symbols:");
    for symbol in info.symbols.iter().take(5) {
        println!(
            "  {} - Status: {}, Price precision: {}, Qty precision: {}",
            symbol.symbol, symbol.status, symbol.price_precision, symbol.quantity_precision
        );
    }
    if info.symbols.len() > 5 {
        println!("  ... and {} more", info.symbols.len() - 5);
    }
    println!();

    // Ticker prices
    println!("=== Ticker Prices ===");
    let tickers = api.ticker_price(None).await?.into_inner();
    for ticker in tickers.iter().take(5) {
        println!("  {}: {}", ticker.symbol, ticker.price);
    }
    if tickers.len() > 5 {
        println!("  ... and {} more", tickers.len() - 5);
    }
    println!();

    // 24hr ticker for first symbol
    if let Some(first_symbol) = info.symbols.first() {
        println!("=== 24hr Ticker ({}) ===", first_symbol.symbol);
        let ticker = api
            .ticker_24hr(Some(&first_symbol.symbol))
            .await?
            .into_inner();
        println!("  Price change: {}", ticker.price_change);
        println!("  Price change %: {}", ticker.price_change_percent);
        println!("  High: {}", ticker.high_price);
        println!("  Low: {}", ticker.low_price);
        println!("  Volume: {}", ticker.volume);
        println!();

        // Order book
        println!("=== Order Book ({}, depth=5) ===", first_symbol.symbol);
        let book = api
            .order_book(Some(5), &first_symbol.symbol)
            .await?
            .into_inner();
        println!("  Bids:");
        for bid in book.bids.iter().take(3) {
            if bid.len() >= 2 {
                println!("    {} @ {}", bid[1], bid[0]);
            }
        }
        println!("  Asks:");
        for ask in book.asks.iter().take(3) {
            if ask.len() >= 2 {
                println!("    {} @ {}", ask[1], ask[0]);
            }
        }
        println!();

        // Recent trades
        println!("=== Recent Trades ({}) ===", first_symbol.symbol);
        let trades = api
            .recent_trades(Some(5), &first_symbol.symbol)
            .await?
            .into_inner();
        for trade in trades.iter().take(3) {
            let side = if trade.is_buyer_maker { "SELL" } else { "BUY" };
            println!(
                "  {} {} @ {} (id: {})",
                side, trade.qty, trade.price, trade.id
            );
        }
        println!();
    }

    // Account info (if address provided)
    if let Ok(address) = std::env::var("ADDRESS") {
        println!("=== Account Info ({address}) ===");
        match api.account_info(&address).await {
            Ok(account) => {
                let account = account.into_inner();
                println!("  Available balance: {}", account.available_balance);
                println!(
                    "  Total wallet balance: {}",
                    account.total_cross_wallet_balance
                );
                println!("  Total unrealized PnL: {}", account.total_cross_un_pnl);
                println!("  Positions: {}", account.positions.len());
            }
            Err(e) => println!("  Error fetching account: {e}"),
        }
        println!();

        println!("=== Account Balance ({address}) ===");
        match api.account_balance(&address).await {
            Ok(balances) => {
                for balance in balances.into_inner().iter().take(5) {
                    println!(
                        "  {}: balance={}, available={}",
                        balance.asset, balance.balance, balance.available_balance
                    );
                }
            }
            Err(e) => println!("  Error fetching balances: {e}"),
        }
    } else {
        println!("Tip: Set ADDRESS env var to query account info");
    }

    Ok(())
}
