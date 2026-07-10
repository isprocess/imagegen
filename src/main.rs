#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = imagegen::cli::run_cli().await {
        eprintln!("imagegen: error: {error:#}");
        std::process::exit(1);
    }
}
