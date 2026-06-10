mod anime;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    #[command(visible_alias = "a")]
    Anime(anime::AnimeCommand),
}

fn main() {
    let cli = Cli::parse();
    let ret = match cli.command {
        SubCommands::Anime(command) => command.run(),
    };
    match ret {
        Ok(Some(s)) => println!("{}", s),
        Ok(None) => {}
        Err(e) => eprintln!("{:?}", e),
    }
}
