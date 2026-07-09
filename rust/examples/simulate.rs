use bullet_rust_sdk::{
    codegen::types::SimulateParameters,
    types::bullet_exchange_interface::{
        message::UserAction, transaction::UniquenessData, types::MarketId,
    },
    Client, Keypair, RuntimeCall, Transaction,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keypair = Keypair::generate();
    let client = Client::builder()
        .network("mainnet")
        .keypair(keypair)
        .build()
        .await?;

    let call =
        RuntimeCall::Exchange(UserAction::CancelMarketOrders { market_id: MarketId(0), sub_account_index: None }.into());

    
    let call = serde_json::to_value(call)?
        .as_object()
        .expect("is an object")
        .clone();
    let uniqueness = serde_json::to_value(UniquenessData::Nonce(0))?
        .as_object()
        .expect("is an object")
        .clone();
    let params = SimulateParameters {
        call,
	sender: "00decda5fb9066278c98d6964648eb954fb4a939b72e891b411ae2fbae33f277".to_string(),
        sequencer: None,
        tx_details: None,
        uniqueness,
    };

    let res = client.simulate(&params).await.expect("simulation failed");
    println!("{res:?}");
    Ok(())
}
