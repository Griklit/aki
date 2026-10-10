use aki::anime::command as anime_command;
use anyhow::Ok;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct AddWithBangumi {
    /// Bangumi 条目 ID。
    bangumi_id: i64,
}

#[derive(Args)]
pub struct UpdateWithBangumi {
    /// Bangumi 条目 ID。
    bangumi_id: i64,
}

#[derive(Args)]
pub struct BuildIndex;

#[derive(Args)]
pub struct Search {
    /// 检索关键词（匹配标题）。
    query: String,
    /// 返回结果数量上限。
    #[arg(short, long, default_value = "16")]
    limit: usize,
}

#[derive(Args)]
pub struct ExportState {
    /// 输出的 SQLite 文件（必填，anime_state 表每次重建）。
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Args)]
pub struct ImportState {
    /// 读取的 SQLite 文件（必填）。
    #[arg(short, long)]
    input: PathBuf,
    /// 只显示将要发生的变化，不写入。
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
pub enum AnimeSubCommands {
    /// 按 Bangumi 条目 ID 添加动画。
    AddWithBangumi(AddWithBangumi),
    /// 按 Bangumi 条目 ID 重新拉取并刷新动画数据（不改动 state）。
    UpdateWithBangumi(UpdateWithBangumi),
    /// 重建动画索引（fjall：bangumi-id 与 title 两个 keyspace）。
    BuildIndex(BuildIndex),
    /// 按标题检索动画：走 title 索引，索引未建时退回扫数据文件。
    Search(Search),
    /// 把所有动画的 state 导出到 SQLite 文件。
    ExportState(ExportState),
    /// 从 export-state 生成的 SQLite 文件导入 state。
    ImportState(ImportState),
    /// 校验动画数据文件结构。
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
                    "indexed {} bangumi ids and {} titles",
                    result.bangumi_id_to_meta_id, result.title_to_meta_id
                )))
            }
            Self::Search(args) => {
                let animes = anime_command::search(&args.query, args.limit);
                Ok(Some(
                    animes
                        .into_iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }
            Self::ExportState(args) => {
                let (path, count) = anime_command::export_state(&args.output)?;
                Ok(Some(format!(
                    "exported {} anime states to {}",
                    count,
                    path.display()
                )))
            }
            Self::ImportState(args) => {
                let result = anime_command::import_state(&args.input, args.dry_run)?;
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
