#[tokio::main]
async fn main() -> anyhow::Result<()> {
    hibp_json_server::run().await
}
