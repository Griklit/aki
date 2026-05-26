use aki::anime_command;
use anyhow::Ok;
use clap::{Args, Parser, Subcommand};

#[derive(Args)]
pub struct AddWithBangumi {
    bangumi_id: i64,
}

#[derive(Args)]
pub struct BuildIndex;

#[derive(Args)]
pub struct Search {
    /// Search query (searches across titles and summaries)
    query: String,
    /// Maximum number of results to return
    #[arg(short, long, default_value = "16")]
    limit: usize,
}

#[derive(Subcommand)]
pub enum AnimeSubCommands {
    AddWithBangumi(AddWithBangumi),
    /// Build a full-text search index for all anime entries.
    /// The index is stored at {anime_data_path}/.aki/tantivy-index/
    BuildIndex(BuildIndex),
    /// Full-text search across anime titles and summaries.
    /// Requires an index to be built first with `build-index`.
    Search(Search),
}

impl AnimeSubCommands {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        match self {
            Self::AddWithBangumi(args) => {
                let runtime = tokio::runtime::Runtime::new()?;
                let client = reqwest::Client::new();
                let anime =
                    runtime.block_on(anime_command::add_with_bangumi(&client, args.bangumi_id))?;
                Ok(Some(anime.to_string()))
            }
            Self::BuildIndex(_) => {
                let count = anime_command::build_index()?;
                Ok(Some(format!("indexed animes: {}", count)))
            }
            Self::Search(args) => {
                let animes = anime_command::search(&args.query, args.limit)?;
                Ok(Some(
                    animes
                        .into_iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }
        }
    }
}

#[derive(Parser)]
#[command(disable_help_subcommand = true)]
pub struct AnimeCommand {
    #[clap(subcommand)]
    command: AnimeSubCommands,
}

impl AnimeCommand {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        self.command.run()
    }
}
