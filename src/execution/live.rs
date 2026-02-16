use std::str::FromStr;
use polymarket_rs::{
    AuthenticatedClient, OrderBuilder, PrivateKeySigner,
    SignatureType, TradingClient,
};

const CLOB_HOST: &str = "https://clob.polymarket.com";
const POLYGON_CHAIN_ID: u64 = 137;

pub async fn load_trading_client() -> anyhow::Result<TradingClient> {
    dotenvy::dotenv().ok();

    let private_key = std::env::var("PRIVATE_KEY")
        .expect("PRIVATE_KEY environment variable not set");
    let signer = PrivateKeySigner::from_str(&private_key)
        .expect("invalid PRIVATE_KEY");

    let auth_client = AuthenticatedClient::new(
        CLOB_HOST,
        signer.clone(),
        POLYGON_CHAIN_ID,
        None,
        None,
    );
    let api_creds = auth_client.create_or_derive_api_key().await?;

    let order_builder = OrderBuilder::new(
        signer.clone(),
        Some(SignatureType::Eoa),
        None,
    );

    let trading_client = TradingClient::new(
        CLOB_HOST,
        signer,
        POLYGON_CHAIN_ID,
        api_creds,
        order_builder,
    );

    Ok(trading_client)
}