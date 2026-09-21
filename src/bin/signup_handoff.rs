use marketplace_verified_handoff::{register_and_handoff, InfraiClient, MarketplaceSignup};
use std::{env, process};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("signup handoff failed: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or(
        "pass the marketplace signup as the first JSON argument; see README.md for the shape",
    )?;
    let signup: MarketplaceSignup = serde_json::from_str(&input)?;
    let client = InfraiClient::from_env()?;
    let receipt = register_and_handoff(&client, signup).await?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
