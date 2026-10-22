use aki::source::bangumi::command as bangumi_command;
use aki::source::bangumi::{Season, SubjectType};
use aki::{CONFIG, Language};
use clap::{Args, Parser, Subcommand};
use std::str::FromStr;

/// `--type` 的取值：条目类型。
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum SubjectTypeArg {
    /// 书籍
    Book,
    /// 动画
    Anime,
    /// 音乐
    Music,
    /// 游戏
    Game,
    /// 三次元
    Real,
}

impl SubjectTypeArg {
    fn to_type(self) -> SubjectType {
        match self {
            Self::Book => SubjectType::Book,
            Self::Anime => SubjectType::Anime,
            Self::Music => SubjectType::Music,
            Self::Game => SubjectType::Game,
            Self::Real => SubjectType::Real,
        }
    }
}

/// `--sort` 的取值：排序规则。
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum SortArg {
    /// 按匹配程度（默认）
    Match,
    /// 按收藏人数
    Heat,
    /// 按排名
    Rank,
    /// 按评分
    Score,
}

impl SortArg {
    fn to_sort(self) -> bangumi_command::SearchSort {
        match self {
            Self::Match => bangumi_command::SearchSort::Match,
            Self::Heat => bangumi_command::SearchSort::Heat,
            Self::Rank => bangumi_command::SearchSort::Rank,
            Self::Score => bangumi_command::SearchSort::Score,
        }
    }
}

#[derive(Args)]
pub struct SearchSubject {
    /// 检索关键词
    keyword: Option<String>,
    /// 返回条数上限（接口最多返回 20 条）。
    #[arg(short, long, default_value_t = bangumi_command::DEFAULT_LIMIT)]
    limit: u32,
    /// 跳过前多少条
    #[arg(long, default_value_t = 0)]
    offset: u32,
    /// 排序规则（默认 match）
    #[arg(short, long, value_enum)]
    sort: Option<SortArg>,
    /// 只要这些条目类型，可重复给出（多值之间是「或」）
    #[arg(short = 't', long = "type", value_enum)]
    types: Vec<SubjectTypeArg>,
    /// 标签筛选，可重复给出（多值之间是「且」）
    #[arg(long)]
    tag: Vec<String>,
    /// 播出/发售日期区间，可重复给出（多值之间是「且」），如 `>=2026-10-01`、`<2027-01-01`
    #[arg(long)]
    air_date: Vec<String>,
    /// 评分区间，可重复给出（多值之间是「且」），如 `>=7`、`<9`
    #[arg(long)]
    rating: Vec<String>,
    /// 只看某一季的新番：`current`（本季，默认）/ `last` / `next` / `2026-10` / `2026-Q4`。
    ///
    /// 给了它就默认只搜动画（-t anime）并按排名排序（-s rank），显式给的 -t / -s 优先。
    #[arg(long, num_args = 0..=1, default_missing_value = "current", value_name = "SEASON")]
    season: Option<String>,
    /// 只要 R18 条目。
    #[arg(long)]
    nsfw: bool,
}

#[derive(Subcommand)]
pub enum BangumiSubCommands {
    /// 按关键词搜索 Bangumi 条目（关键词可省，只用筛选条件也能搜）。
    SearchSubject(SearchSubject),
}

impl BangumiSubCommands {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        match self {
            Self::SearchSubject(args) => {
                let season = args.season.as_deref().map(Season::from_str).transpose()?;
                // --season 只是把季度翻译成一个 air_date 区间，用户自己给的区间不会被丢掉。
                let mut air_date = args.air_date.clone();
                if let Some(season) = season {
                    air_date.extend(season.air_date_filter());
                }
                // 没显式给 -t 时，「某一季的新番」自然指动画。
                let types: Vec<i64> = if args.types.is_empty() && season.is_some() {
                    vec![SubjectType::Anime.code()]
                } else {
                    args.types
                        .iter()
                        .map(|r#type| r#type.to_type().code())
                        .collect()
                };
                let sort = args.sort.map(SortArg::to_sort).unwrap_or_else(|| {
                    if season.is_some() {
                        bangumi_command::SearchSort::Rank
                    } else {
                        bangumi_command::SearchSort::Match
                    }
                });
                // 接口把「未上榜」的 rank 记为 0，且按 rank 升序排，于是这些条目会挤在
                // 最前面；所以真要按排名看，就得把未上榜的排除掉。
                let rank =
                    (sort == bangumi_command::SearchSort::Rank).then(|| vec![">0".to_string()]);
                let filter = bangumi_command::SearchSubjectsFilter {
                    types: (!types.is_empty()).then_some(types),
                    tag: (!args.tag.is_empty()).then(|| args.tag.clone()),
                    air_date: (!air_date.is_empty()).then_some(air_date),
                    rating: (!args.rating.is_empty()).then(|| args.rating.clone()),
                    rank,
                    nsfw: args.nsfw.then_some(true),
                    ..bangumi_command::SearchSubjectsFilter::default()
                };
                // 关键词可省：接口把空关键词当成「不按关键词匹配」，只留筛选条件。
                let keyword = args.keyword.clone().unwrap_or_default();
                let request = bangumi_command::SearchSubjects {
                    sort: Some(sort),
                    filter: Some(filter),
                    limit: args.limit,
                    offset: args.offset,
                    ..bangumi_command::SearchSubjects::new(keyword)
                };

                let runtime = tokio::runtime::Runtime::new()?;
                let client = reqwest::Client::new();
                let result =
                    runtime.block_on(bangumi_command::search_subjects(&client, &request))?;
                Ok(Some(format_search(&result)))
            }
        }
    }
}

/// 把搜索结果整理成人看的几行。
fn format_search(result: &bangumi_command::SubjectSearchResult) -> String {
    let mut lines = vec![format!(
        "total {} (limit {}, offset {})",
        result.total, result.limit, result.offset
    )];
    if result.data.is_empty() {
        lines.push("  no subject matched".to_string());
        return lines.join("\n");
    }
    let language = locale_language();
    for subject in &result.data {
        let r#type = subject.r#type_enumed().map_or("?", SubjectType::as_str);
        let date = subject
            .date
            .map_or_else(|| "unknown".to_string(), |date| date.to_string());
        let name = match language {
            Some(language) => {
                let localized = subject.localized_name(language);
                if localized == subject.name {
                    subject.name.clone()
                } else {
                    format!("{localized}（{}）", subject.name)
                }
            }
            None => subject.name.clone(),
        };
        lines.push(format!(
            "{:>7} - [{:^10}][{:^5}] {} - #{} {}",
            subject.id, date, r#type, name, subject.rating.rank, subject.rating.score
        ));
    }
    lines.join("\n")
}

/// 系统 locale 对应的语言，认不出来时返回 `None`。
fn locale_language() -> Option<Language> {
    let locale = &CONFIG.locale;
    Language::from_str(locale).ok().or_else(|| {
        match locale
            .split(['-', '_'])
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("zh") => Some(Language::ZhCn),
            Some("ja") => Some(Language::JaJp),
            Some("en") => Some(Language::EnUs),
            _ => None,
        }
    })
}

#[derive(Parser)]
#[command(disable_help_subcommand = true)]
pub struct BangumiCommand {
    #[clap(subcommand)]
    command: BangumiSubCommands,
}

impl BangumiCommand {
    pub fn run(&self) -> anyhow::Result<Option<String>> {
        self.command.run()
    }
}
