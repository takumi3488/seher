mod cli;

use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = cli::Args::parse();
    cli::run(args).await;
}
