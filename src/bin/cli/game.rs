use aki::game_command::{self, SyncResult};
use anyhow::Ok;
use clap::{Args, Parser, Subcommand};

#[derive(Args)]
pub struct AddWithSteam {
    /// Steam appid（商店链接 /app/<appid>/ 里的数字）。
    appid: u32,
}

#[derive(Args)]
pub struct SyncOwnedGames {
    /// Only fill in existing entries; do not create data files for owned games
    /// that have none yet.
    #[arg(long)]
    no_create: bool,
    /// Print every owned game instead of only the ones that changed or failed.
    #[arg(long)]
    all: bool,
}

#[derive(Subcommand)]
pub enum GameSubCommands {
    /// Add a game entry with a Steam appid. The raw store response is saved to
    /// `database/steam/appdetails{appid}.json` first, then the game data file is
    /// written to `database/game/`.
    AddWithSteam(AddWithSteam),
    /// Pull the Steam library (`GetOwnedGames`), save the raw response to
    /// `database/steam/owned-games.json`, then fill in the `state` fields that
    /// are still empty (`played_seconds`, `last_played_at`). Values you typed
    /// yourself are never overwritten. Owned games without a data file get one
    /// created (one `appdetails` request each); pass `--no-create` to skip that.
    SyncOwnedGames(SyncOwnedGames),
    /// Verify the structural correctness of the game data file.
    Verify,
}

impl GameSubCommands {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        match self {
            Self::AddWithSteam(args) => {
                let runtime = tokio::runtime::Runtime::new()?;
                let client = reqwest::Client::new();
                let game = runtime.block_on(game_command::add_with_steam(&client, args.appid))?;
                Ok(Some(game.to_string()))
            }
            Self::SyncOwnedGames(args) => {
                let runtime = tokio::runtime::Runtime::new()?;
                let client = reqwest::Client::new();
                let result =
                    runtime.block_on(game_command::sync_owned_games(&client, !args.no_create))?;
                Ok(Some(format_sync(&result, args.all)))
            }
            Self::Verify => Ok(Some("game verify is not implemented yet".to_string())),
        }
    }
}

/// 把同步结果整理成人看的几行。
fn format_sync(result: &SyncResult, all: bool) -> String {
    let mut lines = Vec::new();
    if let Some(path) = &result.raw_path {
        lines.push(format!("raw response saved to {}", path.display()));
    }
    lines.push(format!(
        "owned games: {} (created {}, already tracked {}, filled {}, pending {}, failed {})",
        result.owned,
        result.created(),
        result.existing(),
        result.filled(),
        result.pending(),
        result.failed()
    ));

    let interesting = |record: &game_command::SyncRecord| -> bool {
        all || !record.filled.is_empty() || record.created || record.error.is_some()
    };
    for record in result.records.iter().filter(|r| interesting(r)) {
        let mut line = format!("  {} {}", record.appid, record.name.as_deref().unwrap_or("?"));
        if let Some(seconds) = record.played_seconds {
            line += &format!(" — {seconds}s");
        }
        if let Some(timestamp) = record.last_played_at
            && let Some(date) = aki::game::state::unix_seconds_to_date(timestamp)
        {
            line += &format!(" — last played {date}");
        }
        if !record.filled.is_empty() {
            line += &format!(" — filled {}", record.filled.join(","));
        }
        if let Some(meta_id) = &record.meta_id
            && record.created
        {
            line += &format!(" — new [{meta_id}]");
        }
        if let Some(error) = &record.error {
            line += &format!(" — ERROR {error}");
        }
        lines.push(line);
    }
    lines.join("\n")
}
#[derive(Parser)]
#[command(disable_help_subcommand = true)]
pub struct GameCommand {
    #[clap(subcommand)]
    command: GameSubCommands,
}

impl GameCommand {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        self.command.run()
    }
}
