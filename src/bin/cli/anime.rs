use aki::anime_command;
use anyhow::Ok;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct AddWithBangumi {
    bangumi_id: i64,
}

#[derive(Args)]
pub struct UpdateWithBangumi {
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

#[derive(Args)]
pub struct ExportState {
    /// Output SQLite file (default: {root}/anime-state.db)。
    /// The `anime_state` table in it is dropped and rebuilt on every run;
    /// other tables in the same file are left alone.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImportState {
    /// SQLite file to read (default: {root}/anime-state.db)。
    #[arg(short, long)]
    input: Option<PathBuf>,
    /// Only report what would change; write nothing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
pub enum AnimeSubCommands {
    /// Add an anime entry with a Bangumi ID.
    AddWithBangumi(AddWithBangumi),
    /// Re-fetch an existing anime entry from Bangumi and refresh its data.
    /// Bangumi-provided fields (info, i18n) are overwritten; the `state` you
    /// filled in yourself is left untouched. The data file is renamed when the
    /// Bangumi title or air date changed, so run `build-index` afterwards.
    UpdateWithBangumi(UpdateWithBangumi),
    /// Build the indexes for all anime entries: the full-text search index
    /// ({anime_data_path}/.aki/tantivy-index/) and the bangumi-id index
    /// ({anime_data_path}/.aki/bangumi-id-index.json).
    BuildIndex(BuildIndex),
    /// Full-text search across anime titles and summaries.
    /// Requires an index to be built first with `build-index`.
    Search(Search),
    /// Export every anime's `state` (watch status, score, comment, dates) to a
    /// SQLite file as a single `anime_state` table.
    ExportState(ExportState),
    /// Read a SQLite file written by `export-state` and update each anime's
    /// `state` from it. Only `state` is touched. Rows whose state already
    /// matches are skipped, and nothing is written unless every row is valid.
    ImportState(ImportState),
    /// Verify the structural correctness of the anime data file.
    Verify,
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
            Self::UpdateWithBangumi(args) => {
                let runtime = tokio::runtime::Runtime::new()?;
                let client = reqwest::Client::new();
                let anime = runtime
                    .block_on(anime_command::update_with_bangumi(&client, args.bangumi_id))?;
                Ok(Some(anime.to_string()))
            }
            Self::BuildIndex(_) => {
                let result = anime_command::build_index()?;
                Ok(Some(format!(
                    "indexed animes: {}, bangumi ids: {}",
                    result.animes, result.bangumi_ids
                )))
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
            Self::ExportState(args) => {
                let (path, count) = anime_command::export_state(args.output.as_deref())?;
                Ok(Some(format!(
                    "exported {} anime states to {}",
                    count,
                    path.display()
                )))
            }
            Self::ImportState(args) => {
                let result = anime_command::import_state(args.input.as_deref(), args.dry_run)?;
                let mut lines = vec![format!(
                    "{} {} anime states ({} unchanged)",
                    if args.dry_run {
                        "would import"
                    } else {
                        "imported"
                    },
                    result.updated.len(),
                    result.unchanged
                )];
                for meta_id in result.updated.iter().take(10) {
                    lines.push(format!("  updated: {meta_id}"));
                }
                if result.updated.len() > 10 {
                    lines.push(format!("  … and {} more", result.updated.len() - 10));
                }
                if !result.unknown.is_empty() {
                    lines.push(format!(
                        "  {} row(s) had no matching anime: {}",
                        result.unknown.len(),
                        result
                            .unknown
                            .iter()
                            .take(5)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                Ok(Some(lines.join("\n")))
            }
            Self::Verify => {
                let (count, errors) = anime_command::verify()?;
                let mut ret = format!("verified {} anime files, {} errors", count, errors.len());
                for error in errors {
                    ret += format!("\n[{}] {}", error.path.to_string_lossy(), error.error).as_str()
                }
                Ok(Some(ret))
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
