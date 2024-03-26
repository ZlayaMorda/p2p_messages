use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// period for message sending in seconds
    #[arg(long, default_value_t = 1)]
    period: u32,
    /// Port where peer starts
    #[arg(long)]
    port: String,
    /// Socket address to connect
    #[arg(short, long)]
    connect: Option<String>,
}

fn main() {
    let args = Args::parse();
    println!("Hello {:?}", args);
}
