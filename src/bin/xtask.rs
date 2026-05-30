#[tokio::main]
async fn main() {
    let exit_code = match oxicloud::xtask_runner::run_from_env().await {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    };

    std::process::exit(exit_code);
}
