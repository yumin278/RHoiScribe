#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match rhoiscribe::cli::parse_args(std::env::args())? {
        rhoiscribe::cli::CliCommand::Serve => rhoiscribe::server::run_stdio_server().await,
        rhoiscribe::cli::CliCommand::Help => {
            print!("{}", rhoiscribe::cli::help_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Version => {
            println!("{}", rhoiscribe::cli::version_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::PrintCommand => {
            println!("{}", rhoiscribe::cli::command_path()?.display());
            Ok(())
        }
    }
}
