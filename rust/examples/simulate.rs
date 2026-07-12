use bullet_rust_sdk::codegen::types::SimulateParameters;
use bullet_rust_sdk::types::bullet_exchange_interface::message::UserAction;
use bullet_rust_sdk::types::bullet_exchange_interface::transaction::UniquenessData;
use bullet_rust_sdk::types::bullet_exchange_interface::types::MarketId;
use bullet_rust_sdk::{Client, Keypair, RuntimeCall};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let keypair = Keypair::generate();
    let http_client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .connection_verbose(true)
        .build()
        .expect("need HTTP client");
    let client = Client::builder()
        .reqwest_client(http_client)
        .network("mainnet")
        .keypair(keypair)
        .build()
        .await?;

    let call = RuntimeCall::Exchange(
        UserAction::CancelMarketOrders { market_id: MarketId(0), sub_account_index: None }.into(),
    );

    let call = serde_json::to_value(call)?.as_object().expect("is an object").clone();
    let uniqueness =
        serde_json::to_value(UniquenessData::Nonce(0))?.as_object().expect("is an object").clone();
    let params = SimulateParameters {
        call,
        sender: "00decda5fb9066278c98d6964648eb954fb4a939b72e891b411ae2fbae33f277".to_string(),
        sequencer: None,
        tx_details: None,
        uniqueness,
    };

    for _i in 0..2 {
        let res = client.simulate(&params).await.expect("simulation failed");
        println!("{res:?}");
    }
    Ok(())
}
