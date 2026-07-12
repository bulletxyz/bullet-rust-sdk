use std::sync::{Arc, Mutex};

pub use base64::Engine;
pub use base64::engine::general_purpose::STANDARD as BASE64;
use bullet_rust_sdk::types::bullet_exchange_interface::message::UserAction;
use bullet_rust_sdk::types::bullet_exchange_interface::types::MarketId;
use bullet_rust_sdk::{CallMessage, Client, Keypair, Transaction};
use criterion::{Criterion, criterion_group, criterion_main};
use tokio::runtime::Runtime;

fn criterion_benchmark(c: &mut Criterion) {
    let key = std::env::var("BULLET_ACCOUNT_KEY").expect("no BULLET_ACCOUNT_KEY");
    let key: [u8; 32] = BASE64
        .decode(key)
        .ok()
        .and_then(|x| x.try_into().ok())
        .expect("BULLET_ACCOUNT_KEY env is not a private key");
    let keypair = Keypair::from_bytes(key);

    let rt = Runtime::new().unwrap();
    let http_client =
        reqwest::ClientBuilder::new().http2_prior_knowledge().build().expect("need HTTP client");
    let client = rt.block_on(async {
        Client::builder()
            .reqwest_client(http_client)
            .network(std::env::var("BULLET_NETWORK").unwrap_or("testnet".to_string()))
            .keypair(keypair.clone())
            .build()
            .await
            .expect("no API client")
    });
    let ws = rt.block_on(async { client.connect_ws().call().await.expect("no WS connection") });
    let ws = Arc::from(Mutex::new(ws));
    let call: CallMessage =
        UserAction::CancelMarketOrders { market_id: MarketId(0), sub_account_index: None }.into();
    c.bench_function("ws", |bench| {
        let signed_tx = Transaction::builder()
            .call_message(call.clone())
            .signer(&keypair.clone())
            .client(&client)
            .build()
            .expect("signing");
        bench.to_async(Runtime::new().unwrap()).iter(|| async {
            let mut ws = ws.lock().unwrap();
            ws.order_place(Transaction::to_base64(&signed_tx).unwrap(), None)
                .await
                .expect("order place");
            let _ = core::hint::black_box(ws.recv().await);
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
