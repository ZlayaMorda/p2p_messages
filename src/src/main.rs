use clap::Parser;
use node::errors::NodeError;
use node::node::{Node, NodeBuilder};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::Level;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// period for message sending in seconds
    #[arg(long, default_value_t = 1)]
    period: u64,
    /// Port where node starts
    #[arg(long)]
    port: u16,
    /// Socket address to connect
    #[arg(short, long)]
    connect: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), NodeError> {
    let args: Args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let node: Arc<Node> = Arc::new(
        NodeBuilder::new()
            .address(String::from("127.0.0.1"))
            .port(args.port)
            .period(args.period)?
            .build(),
    );

    let listener: TcpListener = node.bind_address().await?;

    node.clone().connect_to(args.connect).await?;
    node.listen_connections(&listener).await?;
    Ok(())
}
