#![cfg(feature = "integration")]

use bullet_rust_sdk::{MAINNET_URL, TradingApi};

/// Test fixture that provides a configured client and handles cleanup
///
/// This struct uses RAII (Resource Acquisition Is Initialization) pattern:
/// - Setup happens in `new()`
/// - Teardown happens in `Drop::drop()`
struct TestFixture {
    client: TradingApi,
    test_name: &'static str,
}

impl TestFixture {
    /// Create a new test fixture with setup
    async fn new(test_name: &'static str) -> Self {
        let endpoint = std::env::var("BULLET_API_ENDPOINT").unwrap_or(MAINNET_URL.to_string());

        println!("=== Setting up test: {test_name} ===");
        println!("Testing against API endpoint: {endpoint}");

        let client = TradingApi::new(&endpoint, None)
            .await
            .expect("could not connect");

        Self { client, test_name }
    }

    /// Get a reference to the client
    fn client(&self) -> &TradingApi {
        &self.client
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        println!("=== Tearing down test: {} ===", self.test_name);
    }
}

/// Test that we can create a client and call the health endpoint
#[tokio::test]
async fn test_health_endpoint() {
    let fixture = TestFixture::new("test_health_endpoint").await;

    let result = fixture.client().health().await;

    assert!(
        result.is_ok(),
        "Health endpoint should return successfully: {:?}",
        result.err()
    );

    println!("✓ Health check passed");
    // Teardown happens automatically when fixture goes out of scope
}

/// Test that we can fetch exchange info
#[tokio::test]
async fn test_exchange_info() {
    let fixture = TestFixture::new("test_exchange_info").await;

    let result = fixture.client().exchange_info().await;

    assert!(
        result.is_ok(),
        "Exchange info endpoint should return successfully: {:?}",
        result.err()
    );

    let exchange_info = result.unwrap().into_inner();

    println!(
        "Exchange info - Symbols count: {}",
        exchange_info.symbols.len()
    );
    println!(
        "Exchange info - Assets count: {}",
        exchange_info.assets.len()
    );

    assert!(
        !exchange_info.symbols.is_empty(),
        "Exchange info should contain at least one symbol"
    );

    println!("✓ Exchange info test passed");
    // Teardown happens automatically when fixture goes out of scope
}

/// Test that we can fetch ticker prices
#[tokio::test]
async fn test_ticker_price() {
    let fixture = TestFixture::new("test_ticker_price").await;

    let result = fixture.client().ticker_price(None).await;

    assert!(
        result.is_ok(),
        "Ticker price endpoint should return successfully: {:?}",
        result.err()
    );

    let tickers = result.unwrap().into_inner();
    println!("Ticker prices count: {}", tickers.len());

    if !tickers.is_empty() {
        println!(
            "First ticker: symbol={}, price={}",
            tickers[0].symbol, tickers[0].price
        );
    }

    println!("✓ Ticker price test passed");
    // Teardown happens automatically when fixture goes out of scope
}

/// Test WebSocket subscribe and unsubscribe with request ID matching
#[tokio::test]
async fn test_websocket_subscribe_unsubscribe() {
    use bullet_rust_sdk::types::RequestId;
    use bullet_rust_sdk::ws::models::{ServerMessage, TaggedMessage};
    use bullet_rust_sdk::{OrderbookDepth, Topic};

    let fixture = TestFixture::new("test_websocket_subscribe_unsubscribe").await;

    // First, get a valid symbol from exchange info
    let symbol = match fixture.client().exchange_info().await {
        Ok(info) => info
            .into_inner()
            .symbols
            .first()
            .expect("No symbols available")
            .symbol
            .clone(),
        Err(e) => {
            println!("⚠ Skipping test - exchange info not available: {e}");
            println!("  This usually means the rollup backend isn't connected");
            return;
        }
    };

    println!("Using symbol: {}", symbol);

    // Connect to WebSocket
    let mut ws = fixture
        .client()
        .connect_ws()
        .await
        .expect("Failed to connect to WebSocket");

    println!("✓ Connected to WebSocket");

    // Subscribe to a topic
    let topic = Topic::depth(&symbol, OrderbookDepth::D10);
    let subscribe_request_id = RequestId::new(1);
    ws.subscribe([topic.clone()], Some(subscribe_request_id))
        .await
        .expect("Failed to send subscribe");

    println!("✓ Sent subscribe request (id: {})", subscribe_request_id);

    // Wait for subscribe confirmation
    let mut subscribe_confirmed = false;
    for _ in 0..10 {
        let msg = ws.recv().await.expect("Failed to receive message");

        if let Some(response_id) = msg.request_id() {
            if response_id == subscribe_request_id {
                match &msg {
                    ServerMessage::Tagged(TaggedMessage::Subscribe(result)) => {
                        assert_eq!(result.result, "success");
                        subscribe_confirmed = true;
                        println!("✓ Subscribe confirmed (id: {})", response_id);
                        break;
                    }
                    ServerMessage::Tagged(TaggedMessage::Error(err)) => {
                        panic!(
                            "Subscribe failed with error: {} (code: {})",
                            err.error.message(),
                            err.error.code()
                        );
                    }
                    _ => {}
                }
            }
        }

        // If we get market data, that's also a sign subscription worked
        if matches!(msg, ServerMessage::DepthUpdate(_)) {
            println!("  Received depth update (subscription is active)");
        }
    }

    assert!(subscribe_confirmed, "Subscribe confirmation not received");

    // Unsubscribe from the topic
    let unsubscribe_request_id = RequestId::new(2);
    ws.unsubscribe([topic], Some(unsubscribe_request_id))
        .await
        .expect("Failed to send unsubscribe");

    println!(
        "✓ Sent unsubscribe request (id: {})",
        unsubscribe_request_id
    );

    // Wait for unsubscribe confirmation
    let mut unsubscribe_confirmed = false;
    for _ in 0..10 {
        let msg = ws.recv().await.expect("Failed to receive message");

        if let Some(response_id) = msg.request_id() {
            if response_id == unsubscribe_request_id {
                match &msg {
                    ServerMessage::Tagged(TaggedMessage::Unsubscribe(result)) => {
                        assert_eq!(result.result, "success");
                        unsubscribe_confirmed = true;
                        println!("✓ Unsubscribe confirmed (id: {})", response_id);
                        break;
                    }
                    ServerMessage::Tagged(TaggedMessage::Error(err)) => {
                        panic!(
                            "Unsubscribe failed with error: {} (code: {})",
                            err.error.message(),
                            err.error.code()
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    assert!(
        unsubscribe_confirmed,
        "Unsubscribe confirmation not received"
    );

    println!("✓ WebSocket subscribe/unsubscribe test passed");
}

/// Test WebSocket list_subscriptions
#[tokio::test]
async fn test_websocket_list_subscriptions() {
    use bullet_rust_sdk::Topic;
    use bullet_rust_sdk::types::RequestId;
    use bullet_rust_sdk::ws::models::{ServerMessage, TaggedMessage};

    let fixture = TestFixture::new("test_websocket_list_subscriptions").await;

    // First, get valid symbols from exchange info
    let symbols: Vec<String> = match fixture.client().exchange_info().await {
        Ok(info) => info
            .into_inner()
            .symbols
            .iter()
            .take(2)
            .map(|s| s.symbol.clone())
            .collect(),
        Err(e) => {
            println!("⚠ Skipping test - exchange info not available: {e}");
            println!("  This usually means the rollup backend isn't connected");
            return;
        }
    };

    if symbols.len() < 2 {
        println!(
            "⚠ Skipping test - need at least 2 symbols, got {}",
            symbols.len()
        );
        return;
    }

    println!("Using symbols: {:?}", symbols);

    let mut ws = fixture
        .client()
        .connect_ws()
        .await
        .expect("Failed to connect to WebSocket");

    println!("✓ Connected to WebSocket");

    // Subscribe to a couple of topics
    let topics = [
        Topic::agg_trade(&symbols[0]),
        Topic::book_ticker(&symbols[1]),
    ];

    let subscribe_id = RequestId::new(1);
    ws.subscribe(topics.clone(), Some(subscribe_id))
        .await
        .expect("Failed to subscribe");

    // Wait for subscribe confirmation
    for _ in 0..10 {
        let msg = ws.recv().await.expect("Failed to receive message");
        if msg.request_id() == Some(subscribe_id) {
            match &msg {
                ServerMessage::Tagged(TaggedMessage::Subscribe(_)) => {
                    println!("✓ Subscribed to topics");
                    break;
                }
                ServerMessage::Tagged(TaggedMessage::Error(err)) => {
                    panic!(
                        "Subscribe failed with error: {} (code: {})",
                        err.error.message(),
                        err.error.code()
                    );
                }
                _ => {}
            }
        }
    }

    // List subscriptions
    let list_id = RequestId::new(2);
    ws.list_subscriptions(Some(list_id))
        .await
        .expect("Failed to list subscriptions");

    println!("✓ Sent list_subscriptions request (id: {})", list_id);

    // Wait for list response
    for _ in 0..10 {
        let msg = ws.recv().await.expect("Failed to receive message");

        if msg.request_id() == Some(list_id) {
            if let ServerMessage::Tagged(TaggedMessage::ListSubscriptions(result)) = msg {
                println!("✓ Active subscriptions: {:?}", result.result);
                assert!(
                    result.result.len() >= 2,
                    "Expected at least 2 subscriptions, got: {:?}",
                    result.result
                );
                break;
            }
        }
    }

    println!("✓ WebSocket list_subscriptions test passed");
}
