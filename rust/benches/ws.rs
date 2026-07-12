use criterion::{criterion_group, criterion_main, Criterion};
use bullet_rust_sdk::{
    types::bullet_exchange_interface::{
        message::UserAction, types::MarketId,
    },
    Client, Keypair, CallMessage, Transaction
};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

fn criterion_benchmark(c: &mut Criterion) {
    env_logger::init();

    let keypair = Keypair::generate();
    let rt  = Runtime::new().unwrap();
    let http_client = reqwest::ClientBuilder::new()
	.http2_prior_knowledge()
	.connection_verbose(true)
	.build().expect("need HTTP client");
    let client = rt.block_on(async {
	Client::builder()
	    .reqwest_client(http_client)
	    .network("mainnet")
	    .keypair(keypair.clone())
	    .build()
	    .await.expect("no API client")
    });
    let ws = rt.block_on(async {
	client.connect_ws().call().await.expect("no WS connection")
    });
    let ws = Arc::from(Mutex::new(ws));
    let call: CallMessage =
        UserAction::CancelMarketOrders { market_id: MarketId(0), sub_account_index: None }.into();
    c.bench_function("ws", |bench| {
        let signed_tx = Transaction::builder()
            .call_message(call.clone())
            .signer(&keypair.clone())
            .client(&client)
            .build().expect("signing");
	bench.to_async(Runtime::new().unwrap()).iter(|| async {
	    let mut ws = ws.lock().unwrap();
	    ws.order_place(Transaction::to_base64(&signed_tx).unwrap(), None).await.expect("order place");
	    let _ = core::hint::black_box(ws.recv().await);
	});
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
