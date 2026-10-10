mod anime;
mod bangumi;
mod game;
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
    /// 动画
    #[command(visible_alias = "a")]
    Anime(anime::AnimeCommand),
    /// 游戏
    #[command(visible_alias = "g")]
    Game(game::GameCommand),
    /// Bangumi 接口
    #[command(visible_alias = "b")]
    Bangumi(bangumi::BangumiCommand),
}

fn main() {
    let cli = Cli::parse();
    let ret = match cli.command {
        SubCommands::Anime(command) => command.run(),
        SubCommands::Game(command) => command.run(),
        SubCommands::Bangumi(command) => command.run(),
    };
    match ret {
        Ok(Some(s)) => println!("{}", s),
        Ok(None) => {}
        Err(e) => eprintln!("[!] {e:?}"),
    }
}
