use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), og_loc::error::Error> {
    dotenvy::dotenv().ok();
    let cli = og_loc::Cli::parse();
    cli.run().await
}
