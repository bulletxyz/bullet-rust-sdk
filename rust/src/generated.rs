include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

#[cfg(test)]
mod tests {
    use super::Client;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn dispatch_quote_uses_strings_for_precision_sensitive_values() {
        const GAS_LIMIT: &str = "340282366920938463463374607431768211455";
        const RECIPIENT: &str =
            "0x1111111111111111111111111111111111111111111111111111111111111111";
        const RELAYER: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/bridge/dispatchQuote"))
            .and(query_param("destination_domain", "1399811143"))
            .and(query_param("gas_limit", GAS_LIMIT))
            .and(query_param("recipient_address", RECIPIENT))
            .and(query_param("relayer", RELAYER))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "amount": GAS_LIMIT,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = Client::new(&server.uri())
            .dispatch_quote(1_399_811_143, GAS_LIMIT, RECIPIENT, RELAYER)
            .await
            .unwrap()
            .into_inner();

        let amount: String = response.amount;
        assert_eq!(amount, u128::MAX.to_string());
        server.verify().await;
    }
}
