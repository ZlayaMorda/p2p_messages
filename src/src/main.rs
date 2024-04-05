use clap::Parser;
use node::errors::NodeError;
use node::node::{Node, NodeBuilder};
use std::str::FromStr;
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
    /// Tracing level
    #[arg(short, long, default_value_t = String::from("debug"))]
    level: String,
}

#[tokio::main]
async fn main() -> Result<(), NodeError> {
    let args: Args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(
            Level::from_str(&args.level)
                .expect("Not correct level value, choose from: error, warn, info, debug, trace"),
        )
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

    Arc::clone(&node).connect_to(args.connect).await?;
    Arc::clone(&node).listen_connections(&listener).await?;
    Ok(())
}
