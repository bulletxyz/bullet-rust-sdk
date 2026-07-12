use bullet_rust_sdk::codegen::types::SimulateParameters;
use bullet_rust_sdk::types::bullet_exchange_interface::message::UserAction;
use bullet_rust_sdk::types::bullet_exchange_interface::transaction::UniquenessData;
use bullet_rust_sdk::types::bullet_exchange_interface::types::MarketId;
use bullet_rust_sdk::{Client, Keypair, RuntimeCall};
use criterion::{Criterion, criterion_group, criterion_main};
use tokio::runtime::Runtime;

fn criterion_benchmark(c: &mut Criterion) {
    env_logger::init();

    let keypair = Keypair::generate();
    let rt = Runtime::new().unwrap();
    let http_client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .connection_verbose(true)
        .build()
        .expect("need HTTP client");
    let client = rt.block_on(async {
        Client::builder()
            .reqwest_client(http_client)
            .network("https://rollup.mainnet.bullet.xyz")
            .keypair(keypair)
            .build()
            .await
            .unwrap()
    });
    c.bench_function("simulate", |bench| {
        let call = RuntimeCall::Exchange(
            UserAction::CancelMarketOrders { market_id: MarketId(0), sub_account_index: None }
                .into(),
        );
        let call = serde_json::to_value(call).unwrap().as_object().expect("is an object").clone();
        let uniqueness = serde_json::to_value(UniquenessData::Nonce(0))
            .unwrap()
            .as_object()
            .expect("is an object")
            .clone();
        let params = SimulateParameters {
            call,
            //sender: "0000000000000000000000000000000000000000000000000000000000000000".
            // to_string(),
            sender: "00decda5fb9066278c98d6964648eb954fb4a939b72e891b411ae2fbae33f277".to_string(),
            sequencer: None,
            tx_details: None,
            uniqueness,
        };
        bench.to_async(Runtime::new().unwrap()).iter(|| async {
            let _ = client.simulate(&params).await.expect("simulation failed");
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
