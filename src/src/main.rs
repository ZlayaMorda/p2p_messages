use clap::Parser;
use node::errors::NodeError;
use node::node::{Node, NodeBuilder};
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// period for message sending in seconds
    #[arg(long, default_value_t = 1)]
    period: u64,
    /// Port where node starts
    #[arg(long)]
    port: String,
    /// Socket address to connect
    #[arg(short, long)]
    connect: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), NodeError> {
    let args: Args = Args::parse();

    let node: Arc<Node> = Arc::new(
        NodeBuilder::new()
            .address(String::from("127.0.0.1"))
            .await
            .port(args.port)
            .await
            .period(10)
            .await?
            .build()
            .await,
    );

    let listener: TcpListener = node.bind_address().await?;

    node.clone().connect_to(args.connect).await?;
    node.handle_connections(&listener).await?;
    Ok(())
}
