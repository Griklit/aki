# aki — 项目模块文档

## 项目概述

`aki` 是一个用 Rust 编写的个人追番/追游戏管理工具。核心功能是：
- 以 JSON 文件存储番剧与游戏数据（元信息、观看/游玩状态、评分）
- 从 [Bangumi](https://bgm.tv) 拉取番剧元数据，从 [Steam](https://store.steampowered.com) 拉取游戏元数据
- 通过 CLI 命令行交互管理列表

技术栈：Rust 2024 edition, tokio (async), reqwest (HTTP), serde/serde_json (序列化), clap (CLI), fjall (键值索引).

诊断信息一律走 `stderr` 并带级别符号前缀（`[#] [+] [?] [!] [x]`），命令的正常结果走 `stdout` 不加前缀。完整规则见第十一节「日志约定」。

---

## 目录结构

```
aki/
├── Cargo.toml              # Rust 项目配置，定义库+3个二进制目标
├── database/               # 数据存储根目录
│   ├── aki.toml            # 运行时配置文件
│   ├── anime/              # 番剧 JSON 数据文件
│   │   └── [YYYY-MM-DD] 番名.json
│   ├── game/               # 游戏 JSON 数据文件
│   │   └── [日期] 游戏名.json
│   └── source/             # 数据来源的原始响应（`[bangumi]` / `[steam]` 的 `path` 可改）
│       ├── bangumi/subject/{id}.json    # Bangumi 条目原文
│       └── steam/                       # Steam 响应原文
│           ├── appdetails/{appid}.json
│           └── owned-games.json
├── src/
│   ├── lib.rs              # 库根：模块声明 + 公共 API 重导出
│   ├── config.rs           # 配置加载模块
│   ├── anime/              # 番剧领域模型（核心）
│   ├── game/               # 游戏领域模型（与 anime 同构）
│   ├── source/             # 数据来源集成：source::bangumi / source::steam（各带自己的 command/）
│   ├── meta/               # 元数据/ID 管理
│   ├── common/             # 共享基础类型
│   ├── resource/           # 资源管理（封面等，WIP）
│   ├── utils/              # 工具函数
│   └── bin/                # 可执行入口
│       ├── cli/            # 主 CLI 程序 (aki-cli)
│       ├── scripts/        # 一次性数据迁移脚本
│       └── test.rs         # 本地环境测试
```

---

## 一、库核心 (`src/lib.rs`)

**文件**: [src/lib.rs](src/lib.rs)

库根。声明所有子模块，并通过 `pub use` 将内部类型统一重导出为库的公共 API。

重导出的公共类型：

| 类型 | 来源模块 | 用途 |
|------|----------|------|
| `Anime` | `anime::core` | 番剧聚合根 |
| `AnimeInfo` | `anime::info` | 番剧信息 |
| `AnimeInfoInternationalization` | `anime::info` | 番剧信息国际化翻译 |
| `AnimeScore` | `anime::score` | 番剧评分 |
| `AnimeState` | `anime::state` | 番剧观看状态 |
| `AnimeWatchStatus` | `anime::state` | 观看状态枚举 |
| `LoadAnimeError` | `anime::core` | 加载错误 |
| `SaveAnimeError` | `anime::core` | 保存错误 |
| `anime_command` | `anime::command` | 番剧命令模块 |
| `Game` | `game::core` | 游戏聚合根 |
| `GameInfo` | `game::info` | 游戏信息 |
| `GameInfoInternationalization` | `game::info` | 游戏信息国际化翻译 |
| `GameRelease` | `game::info` | 发售日期（支持多种粒度） |
| `GamePlatform` | `game::info` | 平台枚举 |
| `GameAchievements` | `game::state` | 成就进度（已解锁/总数） |
| `GameState` | `game::state` | 游玩状态 |
| `GamePlayStatus` | `game::state` | 游玩状态枚举 |
| `LoadGameError` / `SaveGameError` | `game::core` | 加载/保存错误 |
| `game_command` | `game::command` | 游戏命令模块 |
| `BangumiSubject` | `bangumi::subject` | Bangumi 条目 |
| `BangumiSubjectType` | `bangumi::subject` | Bangumi 条目类型 |
| `Language` | `common::language` | 语言枚举 |
| `Score` | `common::score` | 评分 trait |
| `CONFIG` | `config` | 全局配置（懒加载） |
| `Meta` | `meta` | 元数据容器 |
| `MetaData` | `meta::meta_data` | 元数据信息 |
| `MetaId` | `meta::meta_id` | 带命名空间的 ID |
| `Namespace` | `meta::namespace` | 命名空间 trait |
| `ImageType` | `resource::image_type` | 图片类型枚举 |
| `Resource` | `resource::core` | 通用资源 |
| `resource_type` | `resource::resource_type` | 资源类型模块 |

`steam`（`src/steam/`）是 `pub mod`，与 `bangumi` 平级：里面只有「Steam 返回了什么」，不依赖游戏领域模型。

---

## 二、配置 (`src/config/`)

**文件**: [src/config/](src/config/)（`mod.rs` + 每个配置段一个文件）

**功能**: 加载和管理应用配置。

### 核心结构

- **`Config`** — 顶层配置（[config/mod.rs](src/config/mod.rs)）
  - `root: PathBuf` — 配置文件所在目录（即 `database/`）
  - `locale: String` — 系统语言环境（自动检测）
  - `anime: ConfigAnime` / `game: ConfigGame` — 两个领域的配置
  - `bangumi: ConfigBangumi` / `steam: ConfigSteam` — 两个数据来源的配置（与领域**平级**）
- **`ConfigAnime`**（[config/anime.rs](src/config/anime.rs)）
  - `path: PathBuf` — 番剧数据相对于 root 的路径
- **`ConfigGame`**（[config/game.rs](src/config/game.rs)）
  - `path: PathBuf` — 游戏数据相对于 root 的路径
- **`ConfigBangumi`**（[config/bangumi.rs](src/config/bangumi.rs)）
  - `path: PathBuf` — Bangumi 原始响应目录（默认 `source/bangumi`）
  - `bangumi_api: String` — Bangumi API 地址（默认 `api.bgm.tv`），**从原来的 `[anime]` 移到这里**
- **`ConfigSteam`**（[config/steam.rs](src/config/steam.rs)）
  - `path: PathBuf` — 原始响应目录（默认 `source/steam`）
  - `api_key: String` — Web API 密钥，留空则只有商店接口可用
  - `steam_id: Option<String>` — SteamID64，拉玩家数据（已拥有游戏等）时需要
  - `language` / `country` — 商店接口的文案语言与价格分区（默认 `schinese` / `cn`）
  - `api` / `store_api` — 两个 API 的 base 地址
- **`CONFIG: LazyLock<Config>`** — 全局单例，首次访问时加载

### 配置加载流程

1. 在 debug 模式下，自动切换工作目录到 `database/`
2. 读取当前目录的 `aki.toml`
3. 解析 TOML，设置 `root` 和 `locale`
4. 具体路径不在这里解析：各 `Namespace` 用 `root` 拼上配置里自己那一段（见下）

### 路径归属

**`Config` 上不再有任何领域专属路径方法**（`anime_data_path()` / `game_data_path()` / `namespace_data_path()` 这些都已删除），它只保存原始配置。某个路径属于谁，就由谁的 `Namespace` 从最底层的 `CONFIG.root` 反推出来：

| 命名空间 | 目录 | 域内附加路径 |
|----------|------|--------------|
| [`AnimeNamespace`](src/anime/namespace.rs) | `root/{anime.path}` | `index_path()` = `{data}/.aki/fjall-index` |
| [`GameNamespace`](src/game/namespace.rs) | `root/{game.path}` | — |
| [`BangumiNamespace`](src/source/bangumi/namespace.rs) | `root/{bangumi.path}`（默认 `source/bangumi`） | `subject_path(id)` = `{data}/subject/{id}.json` |
| [`SteamNamespace`](src/source/steam/namespace.rs) | `root/{steam.path}`（默认 `source/steam`） | `saved_path("appdetails/{appid}")` |

```rust
impl Namespace for AnimeNamespace {
    fn data_path() -> PathBuf {
        utils::path::normalize(CONFIG.root.join(&CONFIG.anime.path))
    }
}
```

`MetaId::json_data_path()` = `T::data_path()` + 安全化后的 `{id}.json`，所以目录由命名空间决定，`meta` 模块里没有任何领域名字的硬编码，`Config` 里也没有。

四个命名空间各自的目录来源：

| 命名空间 | `kind()` | 目录 |
|----------|----------|------|
| [`AnimeNamespace`](src/anime/namespace.rs) | `anime` | `{root}/{anime.path}` |
| [`GameNamespace`](src/game/namespace.rs) | `game` | `{root}/{game.path}` |
| [`BangumiNamespace`](src/source/bangumi/namespace.rs) | `bangumi` | `{root}/{bangumi.path}`（默认 `source/bangumi`） |
| [`SteamNamespace`](src/source/steam/namespace.rs) | `steam` | `{root}/{steam.path}`（默认 `source/steam`） |

域内更细的路径也由各自的命名空间提供，例如 `BangumiNamespace::subject_path(id)` = `<data_path>/subject/<id>.json`。

### 配置文件格式 (`database/aki.toml`)

```toml
[anime]
path = "./anime"

[game]
path = "./game"

# 数据来源：与 [anime] / [game] 平级，不是 game 的子模块。
[bangumi]
path = "./source/bangumi"
bangumi_api = "https://proxy.scalt.xyz/api.bgm.tv"

[steam]
path = "./source/steam"
api_key = "<Steam Web API key>"
# steam_id = "7656119xxxxxxxxxx"
language = "schinese"
country = "cn"
```

`[bangumi]` / `[steam]` 两段都可以整段省略（`ConfigBangumi` / `ConfigSteam` 都有 `Default`）：`path` 默认就是 `source/bangumi` / `source/steam`。Steam 的密钥/语言/地区以前挂在 `[game.steam]` 下，现在移到平级的 `[steam]`——Steam 是数据来源，不是 `game` 的子模块。配置分文件见下表：

| 文件 | 配置段 |
|------|--------|
| [config/anime.rs](src/config/anime.rs) | `[anime]` |
| [config/game.rs](src/config/game.rs) | `[game]` |
| [config/bangumi.rs](src/config/bangumi.rs) | `[bangumi]` |
| [config/steam.rs](src/config/steam.rs) | `[steam]` |
| [config/mod.rs](src/config/mod.rs) | `Config`、`CONFIG`、`LoadConfigError` |

---

## 三、番剧领域 (`src/anime/`)

**目录**: [src/anime/](src/anime/)

番剧管理的核心领域逻辑，是本项目最主要的模块。

### 3.1 Anime 聚合根 (`src/anime/core.rs`)

**文件**: [src/anime/core.rs](src/anime/core.rs)

**功能**: 番剧的聚合根结构体，组合了所有番剧相关的子数据。

```rust
pub struct Anime {
    pub meta: Meta<AnimeNamespace>,                         // 元数据（ID、创建时间）
    pub info: AnimeInfo,                                    // 基本信息
    pub internationalizations: HashMap<Language, ...>,      // 多语言翻译
    pub state: AnimeState,                                  // 观看状态/评分
    pub bangumi: Option<BangumiSubject>,                    // Bangumi 原始数据
}
```

**方法**:
- `new(...)` — 构造函数
- `load(path)` / `load_with_meta_id(meta_id)` — 从 JSON 文件反序列化加载
- `iter_all()` / `iter_all_with_path()` — 遍历数据目录；后者同时给出每条数据是从哪个文件读出来的，供需要原地改写的命令区分「文件在规范路径上」与「文件名与 `meta.id` 不一致」
- `save(path)` / `save_as_meta_id()` — 序列化保存为 JSON 文件（不允许覆盖已存在文件）
- `Display` trait — 根据系统语言智能显示标题（优先显示翻译名）
- `From<BangumiSubject>` — 从 Bangumi API 返回的条目创建 Anime，自动检测语言并填充国际化信息

**错误类型**:
- `LoadAnimeError` — 加载失败（IO / JSON 解析）
- `SaveAnimeError` — 保存失败（IO / JSON 序列化 / 文件已存在）

### 3.2 AnimeInfo (`src/anime/info.rs`)

**文件**: [src/anime/info.rs](src/anime/info.rs)

**功能**: 番剧的基本信息数据。

```rust
pub struct AnimeInfo {
    pub title: String,              // 原标题
    pub summary: Option<String>,    // 简介
    pub platform: Option<String>,   // 播放平台
    pub air_date: Option<NaiveDate>,// 播出日期
    pub season: Option<u16>,        // 季度
    pub episodes: Option<u16>,      // 总集数
    pub tags: Vec<String>,          // 标签（来自 Bangumi 条目的 tags，按使用人数降序，只留名字）
}
```

`tags` 带 `#[serde(default)]`：早期数据文件里没有这个字段，读的时候当成空列表，不必为了加字段去改历史数据。

```rust
pub struct AnimeInfoInternationalization {
    pub title: Option<String>,      // 翻译标题
    pub summary: Option<String>,    // 翻译简介
}
```

国际化以 `HashMap<Language, AnimeInfoInternationalization>` 形式存储在 Anime 中，支持按语言分别存储标题和简介的翻译。

### 3.3 AnimeScore (`src/anime/score.rs`)

**文件**: [src/anime/score.rs](src/anime/score.rs)

**功能**: 番剧评分模型。

```rust
pub struct AnimeScore {
    pub positive: Option<f32>,       // 正面评分 0.0~1.0
    pub negative: Option<f32>,       // 负面评分 0.0~1.0
    pub recommendation: Option<u8>,  // 推荐度 1~5
}
```

实现了 `Score` trait：
- `score()` 方法综合正面/负面评分，计算公式: `(positive - negative) / 2 + 0.5`，结果钳制在 `[0.0, 1.0]`

### 3.4 AnimeState (`src/anime/state.rs`)

**文件**: [src/anime/state.rs](src/anime/state.rs)

**功能**: 番剧的观看状态追踪。

**`AnimeWatchStatus` 枚举**（kebab-case 序列化）:

| 变体 | 含义 |
|------|------|
| `Plan` | 计划观看 |
| `Watching` | 正在观看 |
| `Watched` | 已看完 |
| `OnHold` | 暂停观看 |
| `Dropped` | 已弃番 |
| `OriginalWorkWatched` | 已看原作 |

```rust
pub struct AnimeState {
    watch_status: AnimeWatchStatus,
    episodes_watched: u16,
    score: AnimeScore,
    comment: Option<String>,
    watch_start_date: Option<NaiveDate>,
    watch_finish_date: Option<NaiveDate>,
}
```

**方法**:
- `watching()` — 标记为"观看中"，自动记录开始日期
- `watched()` — 标记为"已看完"，自动记录完成日期

### 3.5 AnimeNamespace (`src/anime/namespace.rs`)

**文件**: [src/anime/namespace.rs](src/anime/namespace.rs)

**功能**: 为 Anime 实现 `Namespace` trait，标记其命名空间为 `"anime"`。用于元数据 ID 的命名空间隔离。

### 3.6 命令：add_with_bangumi (`src/anime/command/`)

**目录**: [src/anime/command/](src/anime/command/)

**功能**: 通过 Bangumi ID 添加番剧的异步命令。

```rust
pub async fn add_with_bangumi(client: &reqwest::Client, bangumi_id: i64, refresh: bool)
    -> Result<Anime, AddWithBangumiError>
```

流程：
1. 取条目数据：`Subject::load_or_request_with_id(client, bangumi_id, refresh)`——`source/bangumi/subject/<id>.json` 有缓存就直接读本地，`refresh` 为 `true` 才重新请求
2. 将 `BangumiSubject` 转换为 `Anime`
3. 保存为 JSON 文件

### 3.7 命令：update_with_bangumi (`src/anime/command/`)

**文件**: [src/anime/command/update_with_bangumi.rs](src/anime/command/update_with_bangumi.rs)

**功能**: 按 Bangumi ID 找到已有番剧，取条目数据（默认用本地缓存的原始响应）刷新其中的 Bangumi 数据。

```rust
pub async fn update_with_bangumi(client: &reqwest::Client, bangumi_id: i64, refresh: bool)
    -> Result<Anime, UpdateWithBangumiError>
```

流程：
1. 载入 `.aki/fjall-index/` 里的 bangumi-id 索引，按 `bangumi_id` 直接定位番剧；索引未命中或索引项已失效（文件被改名、删除）时回退到遍历数据目录，并把结果写回索引：一条都没有报 `NotFound`，多于一条报 `Ambiguous`（先查再取数据，失败时不会写入任何数据）
2. 取条目数据：`Subject::load_or_request_with_id(client, bangumi_id, refresh)`——默认复用 `source/bangumi/subject/<id>.json` 里的原始响应，**`refresh` 为 `true` 才真的重新请求**（所以「更新」不加 `--refresh` 时用的是上次存下来的那份响应）
3. 用取到的条目刷新该番剧
4. 写入数据文件并同步索引；若 `meta.id` 变化导致改名，先把索引指向新文件，再删除旧文件，新文件名已被**别的条目**占用时报 `TargetExists`（不覆盖）

> **定位返回的是「真正读了哪个文件」，不是「按 `meta.id` 反推的路径」**（`find_exclusive_anime` 用 `iter_all_with_path()`，索引分支也把读的那个路径带回）。这两者在文件被手工改名、或上次改名只写没删时会不一致，而写完之后要删掉的正是前者——只认反推路径的话，删的会是一个不存在（或另一个）文件名，于是新旧两份都留在目录里。
>
> 同一个 `meta.id` 的多份文件被视为**同一份条目的副本**（不算 `Ambiguous`）：定位时优先取规范路径上那份，写完新文件后把旧的那份删掉。目标文件名被同一个条目的另一份副本占着时允许覆盖（`is_same_entry()` = 同 `meta.id` 且同 `bangumi_id`），被别的条目占着才报 `TargetExists`。

刷新规则：

| 字段 | 处理 |
|------|------|
| `info` | 整体替换为 Bangumi 最新数据 |
| `i18n` | **逐语言 + 逐字段**合并，见下 |
| `meta.id` | 按最新「[上映日期] 原名」重算，数据文件名随之改变 |
| `meta.data.title` | 最新 `name_cn` 非空时刷新，空串（条目没有中文名）时保留原值 |
| `meta.data.create_time` | 保留原值 |
| `state` | 用户填写，原样保留 |

`i18n` 合并（`merge_internationalizations`）不能整条语言替换：`From<Subject>` 给出的每个语言条目只带它那一门语言真正有的东西（原名是日文、简介是中文时，日文条目只有 `title`，中文条目只有 `summary`），整条替换会把 Bangumi 这次没提到的字段一起抹掉。规则是：

| 最新条目在这一门语言给的字段 | 处理 |
|------------------------------|------|
| 非空值 | 覆盖，标题与简介以最新条目为准 |
| `None` 或空串 | 保留原值——可能是用户自己补的翻译，不能因为这次没给就清掉 |

于是：用户自备的语言（如英文）不会被碰；两个字段都是空的「空壳语言」不会新建；已有条目也不会因为这次没数据而被删掉。代价是**不会清理历史遗留值**——比如简介从中文变成日文后，旧的中文简介会留着（无法区分它是上一次 Bangumi 写的还是用户写的，宁可留着）。

> `search` 走 `.aki/fjall-index/` 的 `title` keyspace；索引不存在或还没建时可以退回扫数据目录（只读标题），索引只影响快慢，不影响结果。

### 3.8 索引 (`src/anime/index/`)

**目录**: [src/anime/index/](src/anime/index/)

索引就是 `root/{anime.path}/.aki/fjall-index/`（见 [`AnimeNamespace::index_path()`](src/anime/namespace.rs)）下的一个 fjall 数据库，随时可以删除并从数据目录重建。里面是**几个平表 keyspace**，没有倒排、没有分词：

| keyspace | 键 | 值 | 用途 |
|----------|----|----|------|
| `bangumi-id-to-meta-id` | `bangumi_id`（`i64::to_be_bytes`） | `meta.id` | `update-with-bangumi` 直接定位数据文件 |
| `title-to-meta-id` | 小写化的标题 | `meta.id` | `search` 按标题找番剧 |
| `tag-to-meta-id` | 小写化的标签 + `\x1f` + `meta.id` | `meta.id` | `search --tag` 按标签找番剧 |

整个模块只有 `fjall.rs` 一个文件（[src/anime/index/fjall.rs](src/anime/index/fjall.rs)）。

```rust
pub struct AnimeIndex { /* Database + 三个 Keyspace */ }
impl AnimeIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, fjall::Error>;
    pub fn open_or_warn() -> Option<Self>;              // 打不开就打印警告并退化成「没有索引」
    pub fn set_bangumi_id_to_meta_id(&self, bangumi_id: i64, meta_id: &str) -> Result<(), fjall::Error>;
    pub fn get_meta_id_of_bangumi_id(&self, bangumi_id: i64) -> Option<String>;
    pub fn set_title_to_meta_id(&self, title: &str, meta_id: &str) -> Result<(), fjall::Error>;
    pub fn set_tag_to_meta_id(&self, tag: &str, meta_id: &str) -> Result<(), fjall::Error>;
    pub fn meta_ids_of_tag(&self, tag: &str) -> Vec<String>;      // 扫这个标签的键前缀
    pub fn has_tag_entries(&self) -> bool;                        // 区分「真没有这个标签」与「索引是加标签之前建的」
    pub fn set_anime_index(&self, anime: &Anime) -> Result<(), Vec<(&'static str, fjall::Error)>>;
    pub fn set_anime_index_or_warn(&self, anime: &Anime);   // 逐个 keyspace 打印失败原因
    pub fn title_entries(&self) -> impl Iterator<Item = (String, String)>;  // (小写标题, meta.id)
    pub fn build() -> Result<BuildResult, fjall::Error>;
    pub fn flush / flush_or_warn;                       // 批量写入后落盘
}

pub struct BuildResult {
    pub bangumi_id_to_meta_id: usize,
    pub title_to_meta_id: usize,
    pub tag_to_meta_id: usize,
    pub errors: Vec<fjall::Error>,   // 单条写入失败不会中断整体重建
}
```

- 键值都按 UTF-8 存（`meta.id` 原样、`bangumi_id` 用大端字节），取出即可读、不依赖 serde
- 一条番剧有几个标题（**原名 + 各语言翻译，已去重**）就在 `title-to-meta-id` 里占几个键，见 [`Anime::all_titles`]——同一部番的原名与某个译名一字不差时只算一个（否则同一把键会被写两次，`build-index` 报的标题数也虚高）；标题键统一小写、查询也小写化，因此大小写不敏感。空标题会被 `set_title_to_meta_id` 跳过——fjall 不接受空键
- **标签的键里带上 `meta.id`**：标题是「一个键对一个条目」，而标签天然是「一个标签对多个条目」（624 部番 ≈ 18685 个 `(标签, meta.id)` 对），所以键是 `标签\x1fmeta.id`——查一个标签就是 `prefix()` 扫它自己那一段，写入也天然幂等（同一个 `(标签, meta.id)` 重复写只是覆盖），不用「读出来、改一改、写回去」。分隔符用 U+001F，标签与 `meta.id` 里都不会有它；空标签同样跳过
- 打开数据库会**独占目录**，同一目录在同一进程里只应存在一个 `AnimeIndex` 实例，因此各命令都是用完即弃。

**写入时机**：`add-with-bangumi` 保存新番剧后、`update-with-bangumi` 遍历找到番剧后，以及更新导致 `meta.id` 重算后（都走 `set_anime_index_or_warn` + `flush_or_warn`）。

**重建**：`AnimeIndex::build()` 遍历数据目录，先清空三个 keyspace 再整体写入（已删除的番剧不会残留在索引里），单条失败记进 `BuildResult::errors` 而不中断，最后落一次盘；`aki-cli anime build-index` 就是调它（会打印写了多少 bangumi-id / 标题 / 标签键），打开/清空/落盘失败会带着错误返回而不是只打印警告。**注意**：`update-with-bangumi` 改了 `meta.id`、或改了条目的标签后旧键不会自动消失，想让索引立刻干净就跑一次 `build-index`；`tag-to-meta-id` 是后加的，**旧的索引里没有标签表**，`--tag` 会提示重建。

**读取方式**：命中的索引项仍要能读到对应文件、且其 `bangumi_id` 与查询一致才算有效，否则回退到遍历并修复索引。

**容错**：这是可重建的缓存。各命令用 `open_or_warn()` 拿索引，拿不到就是 `None`，定位退化成遍历数据目录、写入被跳过，只打印一行警告——都不会让真正的增删改失败。

#### 搜索 (`src/anime/command/search.rs`)

**文件**: [src/anime/command/search.rs](src/anime/command/search.rs)

**功能**: 按标题匹配字符串（可用标签筛选），[`Anime::all_titles`]（原名 + 各语言翻译标题）全部参与匹配，大小写不敏感。

```rust
pub fn search(query: &str, tags: &[String], limit: usize, verify_index: bool) -> Vec<Anime>
```

三种用法：

| `query` | `tags` | 行为 |
|---------|--------|------|
| 有 | 空 | 按标题搜（原来的行为） |
| 空 | 有 | **只按标签列番剧**：走 `tag-to-meta-id`，按 `meta.id` 顺序（也就是日期顺序）返回 |
| 有 | 有 | 标题搜出来的候选再按标签过滤，多个标签之间是「且」 |

- 标签先 `normalize_tags()`（去空白、小写、去重、排序）再查，所以大小写与首尾空格都不敏感；**过滤放在截断 `limit` 之前**，`--tag` 不会让一页少几条
- 标题侧先扫 `title-to-meta-id` keyspace（约 1240 个标题键，一次全表扫描几十微秒），只对最终要返回的那几条去读数据文件
- 标签侧用一个标签一个 `prefix()` 扫（18685 个 `(标签, meta.id)` 键），多标签取交集
- 索引目录不存在、打不开、或者对应的表是空的（没跑过 `build-index`）时，退回扫数据目录；扫文件时只反序列化 `meta.id`、标题与 `info.tags`（`TitleRecord`），不解析整个 `Anime`——完整反序列化在 debug 构建下要慢两个数量级
- 索引可用但**一条都没匹配到**时，默认就到此为止（索引是权威的，不为此多扫一遍文件）。只有 `--verify-index` 打开 `verify_index` 参数时才再扫一遍数据文件确认：索引可能落后于数据目录（手工放过文件、或加了条目没重建索引），真扫到了就提示索引过期，并把文件结果返回
- 走文件分支时按情况打印一行 `[?]`（见第十一节的日志点）。标题用 `IndexLookup`、标签用 `TagLookup`，两者都分成 `Hits`（可能为空）、`Missing`（目录不存在）、`Empty`（表里没数据）、`Unreadable`（打不开，`open_or_warn` 已经报过原因，不再重复），再交给同一个纯函数 `table_hint()` 生成提示正文（打印由调用方负责）
- 分档：完全相同（4.0）> 前缀（3.0）> 包含（2.0），同档内标题越短越靠前（`rank - 字数/1000`）；一条番剧取所有标题里的最高分，按分数降序截取 `limit` 条
- 标题与标签都为空时直接返回空结果（不会把整个库列出来）

> **升级提示**：`tag-to-meta-id` 是后加的 keyspace，加标签之前建的索引里没有它，`--tag` 会打印 `[?] ... has no tag entry ...` 并退回扫文件；跑一次 `aki-cli anime build-index` 就好。

> 实测（624 条番剧 / 1240 个标题 / 18685 个标签键）：`search 物语` 走索引 debug 11~17 ms、release 60 ms（release 里进程启动占大头），索引目录 0.6 MB。之前「遍历 + 完整反序列化」的写法是 4.6 s。标题数从早期的 1808 降到 1240 是 [`Anime::all_titles`] 去重的结果——原来每部番的原名与它的某个语言译名常常一字不差（`name` == `name_cn`），同一个键被写了两次。


### 3.9 命令：export_state / import_state (`src/anime/command/`)

**文件**: [export_state.rs](src/anime/command/export_state.rs)、[import_state.rs](src/anime/command/import_state.rs)

**功能**: 把全部番剧的 `state`（用户手填的观看状态、评分、评论、日期）导出成一个 SQLite 文件，供 SQL 查询或其他工具消费；也可以反过来从该文件读回并更新数据文件。

```rust
pub fn export_state(output: &Path) -> Result<(PathBuf, usize), ExportStateError>
pub fn import_state(input: &Path, dry_run: bool) -> Result<ImportStateResult, ImportStateError>
```

- 输出/输入路径**必填**，没有默认位置（曾经有过 `AnimeNamespace::state_db_path()` 提供的 `{root}/anime-state.db` 默认值，已连同该方法一起删除）：导出/导入到哪个文件由调用方明确给出，对应 `--output` / `--input`
- 每次导出都 **`DROP TABLE IF EXISTS anime_state` + 重建**，结果始终是当前状态的完整快照；同一个库里其他表不受影响
- 全部插入包在一个事务里，并写 `PRAGMA user_version = 1` 便于以后改结构
- `meta.id` 是主键。理论上不会重复（文件名就是 sanitize 过的 `meta.id`），万一重复则保留最后一个并打印警告，而不是悄悄少导一条
- 分数**按原样导出、不做范围裁剪**，超出字段约定范围的值能直接看出来；f32 会取到 6 位小数，避免 `0.8` 变成 `0.800000011920929`

表结构：

```sql
CREATE TABLE anime_state (
    meta_id           TEXT PRIMARY KEY,  -- Anime::meta.id
    bangumi_id        INTEGER,
    title             TEXT NOT NULL,     -- info.title（原名）
    title_zh          TEXT,              -- meta.data.title（name_cn）
    watch_status      TEXT NOT NULL,     -- watched / dropped / plan / on-hold / ...
    episodes_watched  INTEGER NOT NULL,
    positive          REAL,              -- AnimeScore::positive
    negative          REAL,
    recommendation    INTEGER,           -- 手填的五档推荐分
    comment           TEXT,
    watch_start_date  TEXT,              -- ISO 日期，无则为 NULL
    watch_finish_date TEXT
)
```

#### import_state 的行为

导入是**会写数据文件**的操作，因此约束写得很死：

1. **只改 `state`**：`info` / `i18n` / `meta` / `bangumi_id` 一律不碰
2. **先全部校验、再落盘**：任何一行不合法（未知 `watch_status`、分数越界、`episodes_watched` 超出 `u16`、日期不是 `YYYY-MM-DD`）就**整体不写**，并列出问题行，不会留下改了一半的数据。这里刻意不用 `AnimeScore::new()`——它会静默 clamp 越界值
3. **只在真的变了才写**：`state` 与文件里完全一致的条目会被跳过，不白写一遍文件、也不动 mtime。所以「导出后原样导入」是纯 no-op
4. **先写临时文件再改名**（`*.json.tmp` → `*.json`），避免写到一半失败把已有数据文件写坏
5. **文件名不是规范名时会清理**：改写目标是 `meta.id` 推算出的规范文件名；如果这条数据其实是从另一个文件名读出来的（手工改过名、或从别处复制来的），写完后会删掉旧文件并打印警告——否则同一个 `meta.id` 会在目录里留下两份
6. DB 里有、但数据目录里找不到对应条目的 `meta.id` 会作为 `unknown` 报告出来，不算错误

依赖 `rusqlite`（`bundled` 特性：SQLite 源码随 crate 一起编译，不需要系统安装 SQLite）。

---

## 三·五、游戏领域 (`src/game/`)

**目录**: [src/game/](src/game/)

与 `anime` 同构的第二个领域：`core` 聚合根 + `info` / `state` / `namespace` + `steam` 转换层 + `command`。当前实现了「按 Steam appid 添加游戏」，评分模型（`score`）还没定，整个文件处于注释状态。

### 3.5.1 Game 聚合根 (`src/game/core.rs`)

```rust
pub struct Game {
    pub meta: Meta<GameNamespace>,
    pub info: GameInfo,
    pub internationalizations: BTreeMap<Language, GameInfoInternationalization>,
    pub state: GameState,
    pub steam_app_id: Option<u32>,
    pub bangumi_id: Option<i64>,   // 预留：Bangumi 的 type=4 也是游戏
}
```

方法：`new()` / `load()` / `load_with_meta_id()` / `save()` / `save_force()` / `save_as_meta_id()` / `save_as_meta_id_force()` / `iter_all()` / `iter_all_with_path()` / `Display`。与 `Anime` 的差别是来源 id 分开存放（`steam_app_id` 与 `bangumi_id`），而不是只留一个 `bangumi_id`。

### 3.5.2 GameInfo 与发售日期 (`src/game/info.rs`)

```rust
pub struct GameInfo {
    pub title: String,
    pub summary: Option<String>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub release: Option<GameRelease>,
    pub platforms: Vec<GamePlatform>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub is_free: bool,
    pub website: Option<String>,
    pub cover: Option<String>,
    pub screenshots: Vec<String>,
    pub external_score: Option<ExternalScore>,  // Metacritic 等第三方分
    pub review_count: Option<u32>,              // Steam 评论总数
}
```

游戏不像番剧那样总有精确到日的播出日期，Steam 会给 `"2024"`、`"2025 年 第 1 季度"`、`"即将推出"` 这类只有粒度的值，所以 `release` 用枚举表示：

| 变体 | 含义 | `date_prefix()` | 能否进 `meta.id` |
|------|------|-----------------|------------------|
| `Exact { date }` | 精确到日 | `2013-07-09` | 是 |
| `YearMonth { year, month }` | 年月 | `2024-07` | 是 |
| `Quarter { year, quarter }` | 季度 | `2025-Q1` | 是 |
| `Year { year }` | 只有年份 | `2024` | 是 |
| `Text { raw }` | 解析不出来，保留原文 | 原文 | 是 |
| `Tba` | 未发售 | `unknown` | 是 |

`GameInfo::release_prefix()` 给 `meta.id` 用，粒度不足时用对应粒度的字符串，而不是硬凑一个日期。

### 3.5.3 GameState (`src/game/state.rs`)

```rust
pub enum GamePlayStatus { Plan, Playing, Played, OnHold, Dropped, Endless }

pub struct GameAchievements { pub unlocked: u32, pub total: u32 }  // 显示为 23/45

pub struct GameState {
    play_status: GamePlayStatus,
    played_seconds: Option<u64>,          // 游玩时长，单位：秒
    achievements: Option<GameAchievements>,
    last_played_at: Option<i64>,          // 最后一次游玩（Unix 秒），Steam 提供
    comment: Option<String>,
    play_start_date: Option<NaiveDate>,
    play_finish_date: Option<NaiveDate>,
}
```

- 时长存**秒**（`played_seconds`），`played_hours()` / `last_played_date()` 只是展示用的换算
- 成就用「已解锁/总数」两个整数，`ratio()` 在 `total == 0` 时返回 `None`（不是 0），`Display` 输出 `23/45`
- `GamePlayStatus` 与 `AnimeWatchStatus` 一样成对提供 `as_str()` / `TryFrom<&str>`，并有测试保证两者与 serde 序列化一致
- `last_played_at` 用 `skip_serializing_if` 省略空值，旧数据文件不用改也能读

**Steam → state 的写入规则**（`state::fill_from_steam`）：**只填 `None` 的字段**，手填的值一律保留。时长按 `Steam 分钟 × 60` 换算成秒；`0` 与负数（Steam 的哨兵值）视为没有数据，不写。

### 3.5.4 Steam → Game 转换 (`src/game/steam.rs`)

```rust
pub fn parse_release(release_date: &steam::model::ReleaseDate) -> GameRelease;
pub fn parse_app_details(body: &str, appid: u32) -> Result<AppDetails, SteamRequestError>;
pub fn app_details_to_game(body: &str, appid: u32) -> Result<Game, SteamRequestError>;
pub async fn request_app_details(client, appid) -> Result<AppDetailsRawStore, SteamRequestError>;
pub fn load_app_details(appid: u32) -> Result<AppDetails, SteamRequestError>;
```

- `impl From<AppDetails> for Game`：拼 `meta.id`（`[日期前缀] 名称`）、按 `Language::detect_from` 填 i18n、把 Steam 的 `genres`/`categories` 映射成 `genres`/`tags`
- `request_app_details` 先落**原始响应原文**，再从原文解析；`parse_app_details` / `load_app_details` 可以完全离线重跑解析（解析逻辑改了不必重新打接口）
- `meta.id` 例子：`[2013-07-09] Dota 2`、`[2020-08-14] Factorio`

### 3.5.5 命令：add-with-steam (`src/game/command/`)

```rust
pub async fn add_with_steam(client: &reqwest::Client, appid: u32) -> Result<Game, AddWithSteamError>
```

流程：请求 `store.steampowered.com/api/appdetails?appids={appid}&l={language}&cc={country}` → 原始响应写到 `database/source/steam/appdetails/{appid}.json` → 解析成 `AppDetails` → 转成 `Game` → 写到 `database/game/[日期] 名称.json`。

> 存放名是 `appdetails/{appid}`，由 [`SteamNamespace::saved_path()`](src/source/steam/namespace.rs) 拼成 `{data_path}/appdetails/{appid}.json`——`appdetails` 现在是真正的子目录（以前那版 `json_path` 会把 `/` 安全化掉，于是所有响应都平铺在 `database/steam/` 下）。

### 3.5.6 命令：sync-owned-games (`src/game/command/sync_owned_games.rs`)

```rust
pub async fn sync_owned_games(client: &reqwest::Client, create_missing: bool)
    -> Result<SyncResult, SyncOwnedGamesError>
```

流程：

1. 调 `IPlayerService/GetOwnedGames`（用配置里的 `steam.steam_id` + `steam.api_key`），整份响应原文写到 `database/source/steam/owned-games.json`
2. 遍历数据目录，建 `steam_app_id → (meta.id, 文件路径)` 映射
3. 对库里每个游戏：
   - **已有条目** → 只补 `played_seconds` / `last_played_at` 里为空的字段，真的改了才回写（`save_force`），没改不碰文件
   - **没有条目且 `create_missing`** → 拉一次 `appdetails` 建条目（名称/发售日期/简介等），再把 Steam 的时长填进 `state`；两条请求之间隔 200 ms
   - **没有条目且 `!create_missing`** → 记为 `pending`，不动数据文件
4. 单条 appdetails 失败只记进 `SyncRecord::error`，**不会中断整个同步**

`SyncResult` 提供 `created()` / `existing()` / `filled()` / `pending()` / `failed()` 五个计数；`--no-create` 只补不建，`--all` 打印每一条（默认只打印有变化或出错的）。

> 同步不会改 `play_status`：Steam 的时长推不出「弃坑/搁置」这类判断，所以状态始终归你手填，同步只负责补时长与最后游玩时间。

---

## 三·六、Steam 接口集成 (`src/steam/`)

**目录**: [src/steam/](src/steam/)

与 `bangumi` 平级：这里只有「Steam 返回了什么」，不涉及游戏领域模型。`steam` 在 `lib.rs` 里是 `pub mod`，因为游戏领域（以及以后的其它领域）都要用它。

### 3.6.1 Endpoint trait (`src/steam/http.rs`)

一个接口 = 路径 + 查询参数 + 响应类型 + 解析方式：

```rust
pub trait Endpoint {
    type Response;
    fn path(&self) -> String;
    fn params(&self) -> Vec<(&'static str, String)>;
    fn use_web_api(&self) -> bool;
    fn needs_api_key(&self) -> bool { false }
    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError>;
}

pub async fn request<E: Endpoint>(client, endpoint: &E) -> Result<E::Response, SteamRequestError>;
```

`request()` 统一负责：拼 base、追加 `key=`（配置为空且接口需要密钥时直接报 `MissingConfig`）、发请求、非 2xx 报 `Status`（带 200 字符的响应片段）、把 JSON 错误补上 URL。加新接口只要实现 trait，不用再写一遍请求与错误处理。

全局配置 `reqwest` 的 `query` 特性用于拼查询参数。

### 3.6.2 已实现的接口 (`src/steam/endpoints.rs`)

| 结构体 | 接口 | 需要密钥 | Base |
|--------|------|----------|------|
| `AppDetailsEndpoint` | `/api/appdetails` | 否 | 商店 |
| `OwnedGamesEndpoint` | `/IPlayerService/GetOwnedGames/v1/` | 是 | Web API |
| `PlayerSummariesEndpoint` | `/ISteamUser/GetPlayerSummaries/v2/` | 是 | Web API |
| `CurrentPlayersEndpoint` | `/ISteamUserStats/GetNumberOfCurrentPlayers/v1/` | 否 | Web API |

`AppDetailsEndpoint` 兼容 Steam 现在会返回的**两种形状**：

- 裸条目 `{"steam_appid":427520,"name":"Factorio",...}`
- 老式外壳 `{"427520":{"success":true,"data":{...}}}`（字段更全，含 `price_overview`、配置需求等）

判据是「顶层是否存在数字 appid 键」；两种情况下都会校验 `steam_appid` 与请求一致（不一致报 `ResponseAppIdMismatch`），`success: false` 报 `AppNotFound`。

### 3.6.3 原始响应落盘 (`src/source/steam/store.rs`)

[`SteamNamespace`](src/source/steam/namespace.rs) 把 `[steam] path` 注册成一个命名空间，并给出存放名到文件的映射：`saved_path(name)` = `{data_path}/{name}.json`。

```rust
pub struct SteamRawStore<T> { pub url: String, pub raw: String, /* marker */ }

impl<T> SteamRawStore<T> {
    pub fn save_named(&self, name: &str) -> Result<PathBuf, io::Error>;  // 写 SteamNamespace::saved_path(name)
    pub fn load_named(name: &str) -> Result<String, io::Error>;
    pub fn load_path(path) -> Result<String, io::Error>;
}
```

`appdetails` 的存放名是 `appdetails/{appid}`，因此文件落在 `database/source/steam/appdetails/{appid}.json`；`owned-games` 落在 `database/source/steam/owned-games.json`。里面存的是**响应原文**——字段顺序、缩进、外壳都保持原样，`serde_json` 只用于读取、不参与写盘；`save_named` 就是把 `SteamRawStore::raw` 那串文本 `fs::write` 下去，和 Bangumi 存 `Subject` 原样响应同理：字段解析错了可以离线重来，补字段也不用重新打接口。

### 3.6.4 脏日期解析 (`src/steam/date.rs`)

- `detect_year()`：任意一个落在 [1900, 2100] 的 4 位数
- `detect_month()`：英文月名（简写/全称，大小写不敏感）
- `detect_numeric_month()` / `detect_quarter()`：中文 `7 月` / `第 1 季度`，以及 `Q1`~`Q4`
- `parse_date()` / `parse_steam_date()`：只有「年 + 月 + 日」都拿得到才返回 `Some`；`"2024"`、`"Jan 2024"`、`"第 1 季度"` 一律 `None`，交给 `GameRelease` 用别的粒度表示
- 已覆盖中英文两种写法的单元测试，包括「不该被误判成某一天」的反例

---

## 四、Bangumi API 集成 (`src/source/bangumi/`)

**目录**: [src/source/bangumi/](src/source/bangumi/)

**功能**: 封装 Bangumi API v0 的条目查询。

### 4.1 Subject (`src/source/bangumi/subject.rs`)

**文件**: [src/source/bangumi/subject.rs](src/source/bangumi/subject.rs)

**功能**: Bangumi API `/v0/subjects/{id}` 返回的完整数据结构。

**核心类型**:

| 类型 | 描述 |
|------|------|
| `Subject` | Bangumi 条目（id, name, name_cn, summary, date, platform, eps, rating, tags 等） |
| `Type` | 条目类型枚举（Book=1, Anime=2, Music=3, Game=4, Real=6） |
| `Images` | 图片 URL 集合（common, grid, large, medium, small） |
| `Rating` | 评分信息（rank, score, total, count 分布） |
| `Collection` | 收藏统计（wish, collect, doing, on_hold, dropped） |
| `Tag` | 标签 |
| `Item`/`Value`/`ValueArrayItem` | 信息框（infobox）的 KV 数据 |

**方法**:
- `load_or_request_with_id(client, id, refresh)` — **取条目数据的入口**：`refresh` 为 `false` 且 `subject_path(id)` 已存在时直接读本地缓存（打印一行 `[+]` 说明这次没联网），否则请求接口并落盘；缓存文件读不出来时打印 `[?]` 并退回请求。`add_with_bangumi` / `update_with_bangumi` 都走它
- `request_with_id(client, id)` — 异步请求 Bangumi API，获取指定 ID 的条目（不做缓存判断，只发请求）
- `save()` / `load_with_id(id)` — 原始响应落到 / 读自 `BangumiNamespace::subject_path(id)`
- `r#type_enumed()` — 将数字类型转为 `Type` 枚举；反向的 `Type::code()` 给出接口用的数字（`5` 不是合法类型），`Type::as_str()` 给出 `book` / `anime` / `music` / `game` / `real`
- `localized_name(language)` — 按语言取名字：中文环境用 `name_cn`，其余语言、以及 `name_cn` 为空时用原名（结果保证非空）

取数据时的错误统一为 [`LoadOrRequestError`]（`Request` / `Save` 两种），所以两个命令的错误枚举里只有一个 `FetchSubject` 变体。

**示例数据**: `subject-example.json` 包含一条完整的 Bangumi API 响应样本，用于测试。`date` 用容错的反序列化：接口在日期未定时给空字符串，这里当成「没有日期」而不是解析失败。

### 4.2 BangumiNamespace (`src/source/bangumi/namespace.rs`)

**文件**: [src/source/bangumi/namespace.rs](src/source/bangumi/namespace.rs)

**功能**: 把 `[bangumi] path`（默认 `database/source/bangumi`）注册成命名空间 `"bangumi"`，并提供域内路径：`subject_path(id)` = `<data_path>/subject/<id>.json`、`subject_dir()`。`Subject::save` / `load_with_id` 只调它，不再直接用 `CONFIG` 拼路径。

### 4.3 命令：search_subject (`src/source/bangumi/command/`)

**目录**: [src/source/bangumi/command/](src/source/bangumi/command/)

**文件**: [search_subject.rs](src/source/bangumi/command/search_subject.rs)、[mod.rs](src/source/bangumi/command/mod.rs)

**功能**: 调 `POST /v0/search/subjects`（接口文档里的「条目搜索」，实验性接口）按关键词搜索条目。只管请求与解析：**不落盘、不打印**，怎么展示交给调用方。

```rust
pub const MAX_LIMIT: u32 = 20;      // 接口侧的上限，请求更大也只回 20 条
pub const DEFAULT_LIMIT: u32 = 10;  // 调用方没指定 limit 时给多少条

pub async fn search_subjects(
    client: &reqwest::Client,
    request: &SearchSubjects,
) -> Result<SubjectSearchResult, SearchSubjectError>
```

请求体与接口文档一一对应：

```rust
pub struct SearchSubjects {
    pub keyword: String,                    // 字段必填，但可以是空串（= 不按关键词匹配）
    pub sort: Option<SearchSort>,           // match(默认) / heat / rank / score
    pub filter: Option<SearchSubjectsFilter>,
    pub limit: u32,                         // 走 query string，不进请求体
    pub offset: u32,                        // 同上
}
```

```rust
pub struct SearchSubjectsFilter {
    pub types: Option<Vec<i64>>,            // 序列化成 "type"，多值之间为「或」
    pub tag: Option<Vec<String>>,           // 「且」
    pub meta_tags: Option<Vec<String>>,     // 「且」，`-标签` 表示排除
    pub air_date: Option<Vec<String>>,      // `>=2020-07-01` 这类比较式
    pub rating: Option<Vec<String>>,        // `>=6`、`<8`
    pub rating_count: Option<Vec<String>>,
    pub rank: Option<Vec<String>>,
    pub nsfw: Option<bool>,
}
```

- **省略即不发送**：所有可选字段都带 `skip_serializing_if = "Option::is_none"`，没给的字段不会以 `null` 的形式发出去（接口把两者当不同意思）
- **`keyword` 可以是空串**：接口把 `""` 当成「不按关键词匹配」，所以只给筛选条件也能搜（例如按 `air_date` 列出某一季的新番）。字段本身仍然照发——接口 schema 要求 `keyword` 必须存在，只是允许为空
- **分页不走请求体**：`limit` / `offset` 由 `search_subjects` 拼成 query string，因此标了 `#[serde(skip)]`；`SearchSubjects::new(keyword)` 给默认值（`DEFAULT_LIMIT` 条、offset 0、不筛选、不排序）
- **`limit` 与上限是两个数**：接口把过大的 `limit` 截到 [`MAX_LIMIT`]（20）——试过请求 50，响应里的 `limit` 仍是 20，会如实反映实际值；[`DEFAULT_LIMIT`]（10）只是调用方不指定时用的默认条数
- 响应解析成 [`SubjectSearchResult`]（= 文档里的 `Paged[Subject]`）：`total` / `limit` / `offset` / `data`，其中 `data` 就是 [`Subject`]，字段与 `/v0/subjects/{id}` 的返回一致（额外出现的 `image` 字段被忽略）
- 错误分三类：`Request`（网络）、`Status`（非 2xx，附带状态码、URL 与 200 字符的响应片段）、`Parse`（响应不是合法的 `Paged[Subject]`）
- 原始响应**不落盘**：搜索没有稳定的身份（同一个关键词的结果会变），留一份原文也没有回放价值

### 4.4 Season — 季度换算 (`src/source/bangumi/season.rs`)

**文件**: [src/source/bangumi/season.rs](src/source/bangumi/season.rs)

**功能**: 番剧的「季度」概念与接口 `air_date` 条件之间的换算。季度指 1 / 4 / 7 / 10 月开播的那三个月，例如 2026 年 1 月番 = `2026-01-01` ~ `2026-03-31`。

```rust
pub struct Season { /* year, quarter(1~4) */ }

impl Season {
    pub fn new(year: i32, quarter: u8) -> Option<Self>   // 年份限 1900~2100
    pub fn of(date: NaiveDate) -> Self                   // 某个日期属于哪一季
    pub fn current() -> Self                             // 今天所在的那一季
    pub fn month(self) -> u32                            // 开播月份：1 / 4 / 7 / 10
    pub fn next(self) / prev(self) -> Self               // 跨年自动进退位
    pub fn air_date_filter(self) -> [String; 2]          // [">=本季首日", "<下季首日"]
}
impl FromStr for Season { type Err = SeasonParseError; }
```

- **上界是开区间**：1 月番写成 `>=2026-01-01` 且 `<2026-04-01`，这样就正好覆盖到 3.31，不需要知道这个月有 30 还是 31 天
- `FromStr` 认这几种写法（大小写不敏感）：`current` / `this` / `now`（本季）、`last` / `prev`（上一季）、`next`（下一季）、`2026-10`（月份写法，只能是 1 / 4 / 7 / 10）、`2026-Q4`（季度序号写法）；`Display` 反向输出 `2026-10` 这种写法，`FromStr` 与 `Display` 可以互相往返
- 解析不出来（`2026-05`、`2026-13`、`abc`…）统一报 [`SeasonParseError::Invalid`]，错误信息里写明可用的写法
- 只做换算，不发请求：`--season` 在 CLI 里就是「`Season::air_date_filter()` 追加进 `air_date` 筛选条件」，用户自己给的 `--air-date` 不会被覆盖

---

## 五、元数据管理 (`src/meta/`)

**目录**: [src/meta/](src/meta/)

**功能**: 通用的元数据/ID 管理系统，支持命名空间隔离。

### 5.1 MetaId (`src/meta/meta_id.rs`)

**文件**: [src/meta/meta_id.rs](src/meta/meta_id.rs)

**功能**: 带命名空间的唯一标识符。

```rust
pub struct MetaId<T: Namespace> {
    namespace: T,   // 序列化时写成 T::kind() 字符串
    pub id: String, // UUID v7 或自定义字符串（如 [2010-10-04] 侵略!イカ娘）
}
```

- `new()` / `with_id(id)` — 生成 UUID v7 或使用自定义 ID
- `json_filename()` — 由 `id` 经文件名安全化得到的文件名（含扩展名）
- `json_data_path()` — 计算 JSON 文件的存储路径: `<data_path>/<sanitized_id>.json`，目录与命名规则全部交给 `T`

`namespace` 字段借 `Namespace` 的序列化辅助函数读写，因此 `MetaId` 是 `meta` 里唯一依赖命名空间序列化格式的类型。

### 5.2 MetaData (`src/meta/meta_data.rs`)

**文件**: [src/meta/meta_data.rs](src/meta/meta_data.rs)

**功能**: 元数据信息。

- `title: Option<String>` — 可读标题
- `create_time: Option<DateTime<Utc>>` — 创建时间

### 5.3 Meta (`src/meta/mod.rs`)

**文件**: [src/meta/mod.rs](src/meta/mod.rs)

**功能**: 元数据容器，把 `MetaId` 与 `MetaData` 组装成领域聚合根里的 `meta` 字段。

- `id: MetaId<T>` — 标识符
- `data: MetaData` — 元数据信息
- 工厂方法: `new(title)` 自动生成 UUID, `with_id(id, title)` 使用自定义 ID
- `json_data_path()` — 转发给 `self.id.json_data_path()`

### 5.4 Namespace trait (`src/meta/namespace.rs`)

**文件**: [src/meta/namespace.rs](src/meta/namespace.rs)

**功能**: 定义命名空间抽象。

```rust
pub trait Namespace {
    fn new() -> Self;
    fn kind() -> &'static str;      // 如 "anime"
    fn root_path() -> PathBuf;      // 数据根目录（CONFIG.root），有默认实现
    fn data_path() -> PathBuf;      // 如 database/anime，各命名空间自己算
}
```

trait 只说三件事：**是哪个命名空间**、**数据根目录在哪**、**这个命名空间的数据放在哪**。

**具体文件名/子目录不放在 trait 上**，由各命名空间的固有方法提供，因为规则各不相同：

| 命名空间 | 固有方法 |
|----------|----------|
| `AnimeNamespace` | `index_path()`（`{data}/.aki/fjall-index`） |
| `BangumiNamespace` | `subject_path(id)` = `{data}/subject/{id}.json`、`subject_dir()` |
| `SteamNamespace` | `saved_path(name)` = `{data}/{name}.json` |

> 曾经这里有 `file_path()` / `json_path()` / `save_json()` 三个通用方法，已经删掉：它们绑死了「平铺一个 json、用文件名安全化当子目录」的假设（`appdetails/{appid}` 会被安全化成 `appdetails{appid}`），也把读写职责混进了路径抽象。现在 `MetaId::json_data_path()` 自己拼 `data_path()` + 安全化后的文件名（安全化仍由 [utils::path](src/utils/path.rs) 提供，防止 `id` 里的 `/` 变成子目录）。

这是「路径由领域自己决定」的落点：`MetaId::json_data_path()` 只依赖 `T::data_path()`，`meta` 模块里不再出现 `anime` 之类的具体名字。新增一个领域（或像 `bangumi` / `steam` 这样的数据来源目录）只需要实现 `kind()` 与 `data_path()`——后者一律从 `root_path()`（即 `CONFIG.root`）出发拼自己那段相对路径。

同时包含序列化/反序列化辅助函数，确保命名空间字符串在序列化时一致。

---

## 六、通用基础类型 (`src/common/`)

**目录**: [src/common/](src/common/)

### 6.1 Language (`src/common/language.rs`)

**文件**: [src/common/language.rs](src/common/language.rs)

**功能**: 支持的语言枚举。

```rust
pub enum Language {
    JaJp,   // 日语 → "ja-JP"
    ZhCn,   // 简体中文 → "zh-CN"
    EnUs,   // 英语 → "en-US"
}
```

**关键方法**:
- `detect_from(text)` — **自动检测文本语言**
  - 日文假名（平假名/片假名）占比 > 5% → 日语
  - 汉字占比 > 10% → 简体中文
  - 否则 → 英语
- `FromStr` — 从 locale 字符串解析
- `Display` — 输出 locale 字符串

### 6.2 Score trait (`src/common/score.rs`)

**文件**: [src/common/score.rs](src/common/score.rs)

**功能**: 定义评分抽象。

```rust
pub trait Score {
    fn score(&self) -> f32;  // 返回 0.0~1.0 的综合评分
}
```

由 `AnimeScore` 实现。

---

## 七、资源管理 (`src/resource/`)

**目录**: [src/resource/](src/resource/)

**功能**: 通用资源/资产管理（目前为早期 WIP 状态，尚未在 Anime 中启用）。

### 7.1 Resource (`src/resource/core.rs`)

**文件**: [src/resource/core.rs](src/resource/core.rs)

**功能**: 泛型资源容器。

```rust
pub struct Resource<T: ResourceType> {
    pub id: uuid::Uuid,
    pub info: T,
}
```

### 7.2 ImageType (`src/resource/image_type.rs`)

**文件**: [src/resource/image_type.rs](src/resource/image_type.rs)

**功能**: 支持的图片格式枚举。

| 变体 | 扩展名 | MIME 类型 |
|------|--------|-----------|
| `Png` | png | image/png |
| `Jpeg` | jpg | image/jpeg |
| `Gif` | gif | image/gif |
| `Webp` | webp | image/webp |
| `Avif` | avif | image/avif |

方法: `extension()`, `from_extension()`, `mime_type()`.

### 7.3 ResourceType / Cover (`src/resource/resource_type.rs`)

**文件**: [src/resource/resource_type.rs](src/resource/resource_type.rs)

**功能**: 定义资源类型 trait 和封面类型。

```rust
pub trait ResourceType {
    fn kind() -> &'static str;
    fn extension(&self) -> &'static str;
}

pub struct Cover(ImageType);  // 封面资源，kind = "cover"
```

注：`Cover` 暂时未在 `AnimeInfo` 中启用（`cover` 字段被注释掉）。

---

## 八、工具函数 (`src/utils/`)

**目录**: [src/utils/](src/utils/)

### 8.1 error (`src/utils/error.rs`)

**文件**: [src/utils/error.rs](src/utils/error.rs)

**功能**: 通用错误类型。

- `EnumVariantNotFound` — 枚举变体未找到，用于 `TryFrom` 转换失败时的统一错误

### 8.2 path (`src/utils/path.rs`)

**文件**: [src/utils/path.rs](src/utils/path.rs)

**功能**: 路径规范化。

```rust
pub fn normalize(path: impl AsRef<Path>) -> PathBuf
```

处理 `.`（当前目录）、`..`（父目录）、多余斜杠，保留超出根目录的 `..` 不消除。支持 Windows 路径前缀。

### 8.3 text (`src/utils/text.rs`)

**文件**: [src/utils/text.rs](src/utils/text.rs)

**功能**: 文本处理。

```rust
pub fn truncate(s: &str, max: usize) -> String
```

把一段文本截到 `max` 字节以内，附在错误信息里当响应片段用。按字节截断但不会把一个 UTF-8 字符切成两半，被截断时补 `…`。Steam 与 Bangumi 的错误分支共用它（原先 [`source::steam::http`](src/source/steam/http.rs) 里有一份私有实现）。

---

## 九、可执行入口 (`src/bin/`)

### 9.1 aki-cli — 主 CLI (`src/bin/cli/`)

**目录**: [src/bin/cli/](src/bin/cli/)

**功能**: 面向用户的主命令行界面，使用 `clap` 构建。

**命令结构**:
```
aki-cli
├── anime (alias: a)
│   ├── add-with-bangumi <bangumi_id> [--refresh]
│   ├── update-with-bangumi <bangumi_id> [--refresh]
│   ├── build-index
│   ├── search <query> [-l <limit>] [--tag <tag>...] [--verify-index]
│   ├── export-state -o <path>
│   ├── import-state -i <path> [--dry-run]
│   └── verify
├── game (alias: g)
│   ├── add-with-steam <appid>
│   ├── sync-owned-games [--no-create] [--all]
│   └── verify
└── bangumi (alias: b)
    └── search-subject [keyword] [-l <limit>] [--offset <n>] [-s <sort>] [-t <type>...] [--tag <tag>...] [--air-date <expr>...] [--rating <expr>...] [--season [<spec>]] [--nsfw]
```

**文件**:
- [mod.rs](src/bin/cli/mod.rs) — CLI 入口，定义 `SubCommands::Anime` / `SubCommands::Game` / `SubCommands::Bangumi` 子命令
- [anime.rs](src/bin/cli/anime.rs) — Anime 子命令，`AddWithBangumi` / `UpdateWithBangumi` 接收 Bangumi ID 与 `--refresh`，创建 tokio runtime 调用对应的异步函数（默认复用本地缓存的原始响应，`--refresh` 才重新请求接口），成功后打印番剧标题；`BuildIndex` 重建 fjall 索引并打印写入的 bangumi-id / 标题 / 标签键数量；`Search` 按标题匹配字符串，`--tag` 可按标签筛选（clap 的 `ArgGroup` 保证「关键词与标签至少给一个」，只给 `--tag` 就是按标签列表）（走 title/tag 索引，没索引时自动退回扫文件；`--verify-index` 让「索引里没匹配到」的查询再扫一遍文件，确认索引有没有落后于数据目录）；`ExportState` / `ImportState` 与 SQLite 互导 `state`
- [game.rs](src/bin/cli/game.rs) — Game 子命令，`AddWithSteam` 接收 Steam appid，创建 tokio runtime 调用 `game_command::add_with_steam`，成功后打印游戏标题与时长/成就；`Verify` 尚未实现
- [bangumi.rs](src/bin/cli/bangumi.rs) — Bangumi 子命令，目前只有 `SearchSubject`：关键词**可省**（省了就只用筛选条件，接口把空关键词当成「不按关键词匹配」，因此 `-t anime --air-date ">=2026-10-01" --air-date "<2027-01-01"` 就能列出某一季的新番），拼出 [`source::bangumi::command::SearchSubjects`](src/source/bangumi/command/search_subject.rs) 后调用 `search_subjects`，再把结果逐行打印成 `{bangumi-id:>7} - [{date:^10}][{type:^5}] {译名}（{原名}） - #{rank} {score}`（如 `   1988 - [2006-11-01][book ] 物语系列（〈物語〉シリーズ） - #117 8.5`）。条目 ID 右对齐到 7 位，日期与类型居中（分别占 10 / 5 位），名称由 [`Subject::localized_name()`](src/source/bangumi/subject.rs) 按系统语言取（中文环境用 `name_cn`，取不到或与原名相同则省掉括号），日期未定时写 `unknown`，排名 0 就是接口给的「未上榜」。`--type` 的取值经 CLI 自己的 clap `ValueEnum` 映射成接口要的数字。`--season` 是查季番的快捷方式：把 [`Season`](src/source/bangumi/season.rs) 解析成一组 `air_date` 条件（`>=本季首日` + `<下季首日`），没显式给 `-t` 时补上 `anime`、没显式给 `-s` 时改成 `rank`；按 `rank` 排序时还会加一条 `rank > 0`——接口把未上榜记成 0 且按升序排，不加这条的话排在前面的全是还没上榜的条目

**使用示例**:
```bash
aki-cli anime add-with-bangumi 475354           # 本地有原始响应就直接用它
aki-cli anime add-with-bangumi 475354 --refresh  # 忽略缓存，重新请求接口
aki-cli anime update-with-bangumi 475354         # 默认用缓存的响应刷新（不联网）
aki-cli anime update-with-bangumi 475354 --refresh  # 重新拉一次再刷新
aki-cli anime build-index                    # 重建 .aki/fjall-index/ 里的 bangumi-id 索引
aki-cli anime search 物语 -l 5               # 按标题直接匹配，不需要先 build-index
aki-cli anime search --tag 奇幻 -l 5         # 只按标签列（需要跑过新版 build-index）
aki-cli anime search --tag 奇幻 --tag SHAFT  # 多个标签是「且」
aki-cli anime search 物语 --tag 奇幻         # 标题 + 标签
aki-cli anime export-state -o /tmp/state.db  # 导出到指定文件（-o/--output 必填）
aki-cli anime import-state -i /tmp/state.db --dry-run  # 先看会改哪些（-i/--input 必填）
aki-cli anime import-state -i /tmp/state.db  # 真写回数据文件
aki-cli game add-with-steam 570             # 拉取 Dota 2 并存原始响应
aki-cli game sync-owned-games --no-create   # 只把「已拥有游戏」里为空的时长/最后游玩补上
aki-cli game sync-owned-games --all         # 顺便给缺失的游戏建条目，并逐条打印
aki-cli bangumi search-subject 物语 -t anime -l 5   # 搜 Bangumi 条目（接口最多回 20 条）
aki-cli bangumi search-subject 命运石之门 --tag 科幻 -s score
aki-cli bangumi search-subject 物语 --offset 20     # 翻下一页
aki-cli bangumi search-subject -t anime --air-date ">=2026-10-01" --air-date "<2027-01-01" -s heat
                                                    # 关键词可省：只看 2026 年 10 月番（本季新番）
aki-cli bangumi search-subject --season             # 本季新番，按排名（上面的快捷写法）
aki-cli bangumi search-subject --season 2026-01     # 2026 年 1 月番（1.1~3.31），按排名
aki-cli bangumi search-subject --season 2026-Q4 -s heat   # 用季度序号写，改按收藏人数排
aki-cli bangumi search-subject --season last        # 上一季；next 是下一季
aki-cli bangumi search-subject 药屋 --season        # 关键词 + 本季
# 或
aki-cli a add-with-bangumi 475354
aki-cli a export-state -o /tmp/state.db
aki-cli g add-with-steam 570
aki-cli g sync-owned-games
aki-cli b search-subject 物语
```

> `game add-with-steam` 需要能访问 `store.steampowered.com`；如果本机走代理，`reqwest` 会读 `HTTPS_PROXY`，没设置代理且直连不通时会报 `http request error`。
>
> `bangumi search-subject` 只读接口、不碰本地数据：它不发 `add-with-bangumi`，搜到的结果要入库还得自己拿 id 再跑 `anime add-with-bangumi <id>`。

### 9.2 scripts — 数据迁移脚本 (`src/bin/scripts/`)

**目录**: [src/bin/scripts/](src/bin/scripts/)

**功能**: 一次性脚本，用于从旧的 `anime.json` 数据库批量迁移数据。

- [mod.rs](src/bin/scripts/mod.rs) — tokio main 入口
- [parse_raw.rs](src/bin/scripts/parse_raw.rs) — 迁移逻辑：
  1. 加载 `database/anime/` 中已有的番剧 JSON 文件
  2. 读取旧格式的 `database/anime.json`（数组）
  3. 对每个条目，通过 Bangumi ID 重新拉取元数据
  4. 结合旧的观看状态/评分创建新的 `Anime` 结构
  5. 保存为单独 JSON 文件，间隔 1 秒（API 限流）

### 9.3 test (`src/bin/test.rs`)

**文件**: [src/bin/test.rs](src/bin/test.rs)

**功能**: 简单调试工具，打印系统语言环境列表。

---

## 十、数据流总结

```
用户输入 (CLI)
    │
    ▼
aki-cli (clap 解析命令)
    │
    ├─ anime_command::add_with_bangumi(bangumi_id, refresh)
    │   │
    │   ├── source::bangumi::Subject::load_or_request_with_id()
    │   │   本地已有 source/bangumi/subject/{id}.json 且没要求 --refresh 就直接读它；
    │   │   否则请求 Bangumi API，返回 JSON → 反序列化为 Subject
    │   │   同时 Subject::save() → BangumiNamespace::subject_path(id)
    │   │                        → database/source/bangumi/subject/{id}.json（响应原文）
    │   │
    │   ├── Anime::from(subject)   ─── 自动语言检测 + 国际化填充
    │   │
    │   └── anime.save_as_meta_id()
    │        │
    │        └── MetaId::json_data_path() = T::data_path() + {id}.json
    │             → AnimeNamespace::data_path() = root/{anime.path}
    │             → database/anime/[YYYY-MM-DD] 番名.json
    │
    └─ game_command::add_with_steam(appid)
        │
        ├── source::steam::request(AppDetailsEndpoint) ◄── store.steampowered.com/api/appdetails
        │   │
        │   ├── SteamRawStore::save_named("appdetails/{appid}")
        │   │    → database/source/steam/appdetails/{appid}.json（响应原文先落盘）
        │   │
        │   └── AppDetailsEndpoint::parse() → AppDetails
        │
        ├── Game::from(app_details)   ─── 发售日期粒度降级 + 语言检测 + 国际化填充
        │
        └── game.save_as_meta_id()
             │
             └── MetaId::json_data_path() = T::data_path() + {id}.json
                  → GameNamespace::data_path() = root/{game.path}
                  → database/game/[日期前缀] 游戏名.json
```

所有落盘路径都只经过各命名空间提供的静态方法（`Namespace::data_path()` 定根目录、各命名空间的固有方法给文件名），因此 `meta` 模块与具体领域完全解耦；Bangumi / Steam 的原始响应各自多注册了一个命名空间，目录由 `[bangumi]` / `[steam]` 的 `path` 决定。

## 十一、日志约定

**功能**: 统一所有诊断信息的格式，让终端输出一眼能分出「结果」和「噪音」。

### 级别与符号

| 符号 | 级别 | 什么时候用 | 现有例子 |
|------|------|------------|----------|
| `[#]` | debug | 只给开发看的中间结果，普通用户不需要 | `[#] parsed real response: [2020-08-14] Factorio`（`game/steam.rs` 测试） |
| `[+]` | info | 正常但有话要说的进展 | `[+] using cached bangumi subject '...' (pass --refresh ...)`（`source/bangumi/subject.rs`，缓存命中时说明这次没联网） |
| `[?]` | warn | 能继续，但结果可能不完整 | `[?] failed to open anime index '...': ...`、`[?] skip: ... not found` |
| `[!]` | error | 这次操作失败，但程序还能正常收尾 | `[!] no anime found with bangumi id 999999999` |
| `[x]` | fatal | 继续不下去，只能退出 | `[x] load config failed: config file 'aki.toml' not found in current directory` |

### 格式规则

1. **符号 + 一个空格 + 正文**：`eprintln!("[?] failed to load anime file '{}': {}", path.display(), e);`
2. **正文小写开头**，句尾不加句号；专有名词（`fjall`、`SQLite`、`meta.id`）保持原样
3. **只写一个前缀**：不要 `"Warning: [?] ..."`、不要 `"Error: ..."` —— 符号本身就表达级别
4. **参数化，不拼字符串**：一律用 `{}` 占位符与 `eprintln!` 的参数列表，别先 `format!` 再打印
5. **不换行**：一条日志一行。太长的多行 `eprintln!` 用 Rust 的 `\` 续行接字符串字面量
6. **路径用 `{}` + `.display()`**：`PathBuf` 不能直接 `{}`（不可控编码），统一 `path.display()`
7. **符号只出现在 `stderr`**：`stdout` 只放命令结果

### 出口分工

- **`stdout`**：命令的结果本身，由 `src/bin/cli/mod.rs` 统一 `println!("{}", s)` 输出，**不带任何符号**。
- **`stderr`**：一切诊断信息。包括 `[x]` 致命错误、`[!]` 命令错误、`[?]` 警告、`[#]` 调试输出。
- 目前没有引入 `log`/`tracing` 之类的日志框架：项目只有一个短命进程的 CLI，直接 `eprintln!` 最省事，也保证 `--help`／管道场景下不互相污染。

### 写新代码时的取舍

| 情况 | 做法 |
|------|------|
| 命令失败（网络、IO、找不到数据） | 返回 `Err`，把包装错误交给上层；**不要**在这里打印 |
| 进程起不来（配置读不到、工作目录切不过去） | `[x] ...` 然后 `std::process::exit(1)`，见 [config/mod.rs](src/config/mod.rs) 的 `CONFIG` |
| 命令错误最终出口 | 只在 [src/bin/cli/mod.rs](src/bin/cli/mod.rs) 打一次 `eprintln!("[!] {e:?}")`，并 `exit(1)`（退出码要让脚本能判断成败） |
| 跳过一条坏数据、继续处理其余 | `[?] ...` 并继续（如 `Anime::iter_all_with_path` 跳过读不动的 JSON） |
| 可选操作失败但不影响用户数据 | `[?] ...` 并继续（如索引读写失败、多余文件清理失败） |
| 会改动用户数据却失败了 | 必须返回 `Err`：宁可命令失败，也不能让用户以为成功了 |
| 只是开发期观察的中间值 | `[#] ...`（测试里常见） |

> 核心原则：**只在「错误不影响用户数据、或需要让用户知道被跳过了」时才降级成 `[?]` 并继续；其余一律返回错误。**

### 现有日志点

全部 `eprintln!` 都在下列文件（改动时保持格式一致）：

| 文件 | 级别 | 内容 |
|------|------|------|
| [config/mod.rs](src/config/mod.rs) | `[x]` | `enter 'database' failed` / `load config failed` |
| [src/bin/cli/mod.rs](src/bin/cli/mod.rs) | `[!]` | `{e:?}`（命令错误统一出口） |
| [anime/mod.rs](src/anime/mod.rs)、[game/mod.rs](src/game/mod.rs) | `[?]` | `failed to load anime/game file` |
| [anime/index/fjall.rs](src/anime/index/fjall.rs) | `[?]` | 索引打开/读取/写入/落盘失败 |
| [anime/command/search.rs](src/anime/command/search.rs) | `[?]` | 索引不存在 / 标题表为空 / 索引没匹配到但文件扫到了（索引可能过期） |
| [source/bangumi/subject.rs](src/source/bangumi/subject.rs) | `[+]`、`[?]` | 用了本地缓存的原始响应（`--refresh` 可重新拉）/ 缓存读不出来，退回请求 |
| [anime/command/export_state.rs](src/anime/command/export_state.rs) | `[?]` | `meta.id` 重复，只导出最后一个 |
| [anime/command/import_state.rs](src/anime/command/import_state.rs) | `[?]` | 数据文件不在规范路径，已改名清理 |
| [game/command/sync_owned_games.rs](src/game/command/sync_owned_games.rs) | `[?]` | 同一 appid 出现在两个文件 / 本地还没有 owned-games.json |
| [game/steam.rs](src/game/steam.rs) | `[?]`、`[#]` | 本地响应文件缺失时跳过 / 解析出的真实响应 |

---

## 十二、依赖说明

| 依赖 | 用途 |
|------|------|
| `serde` / `serde_json` | JSON 序列化/反序列化 |
| `clap` | CLI 参数解析 |
| `reqwest` | HTTP 客户端（调用 Bangumi / Steam 接口），启用 `json` 与 `query` 特性 |
| `tokio` | 异步运行时 |
| `chrono` | 日期时间处理 |
| `uuid` (v7) | 唯一标识符生成 |
| `sanitize-filename` | 文件名安全化 |
| `toml` | 配置文件解析 |
| `anyhow` / `thiserror` | 错误处理 |
| `sys-locale` | 系统语言环境检测 |
| `fjall` | 番剧索引数据库（bangumi-id → meta.id 的键值索引） |
| `rusqlite`（`bundled`） | 导出 state 到 SQLite；SQLite 源码随 crate 编译 |
