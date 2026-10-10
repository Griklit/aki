# aki — 项目模块文档

## 项目概述

`aki` 是一个用 Rust 编写的个人追番/追游戏管理工具。核心功能是：
- 以 JSON 文件存储番剧与游戏数据（元信息、观看/游玩状态、评分）
- 从 [Bangumi](https://bgm.tv) 拉取番剧元数据，从 [Steam](https://store.steampowered.com) 拉取游戏元数据
- 通过 CLI 命令行交互管理列表

技术栈：Rust 2024 edition, tokio (async), reqwest (HTTP), serde/serde_json (序列化), clap (CLI), fjall (键值索引).

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
│   └── steam/              # Steam 接口原始响应（appdetails{appid}.json 等）
├── src/
│   ├── lib.rs              # 库根：模块声明 + 公共 API 重导出
│   ├── config.rs           # 配置加载模块
│   ├── anime/              # 番剧领域模型（核心）
│   ├── game/               # 游戏领域模型（与 anime 同构）
│   ├── bangumi/            # Bangumi API 集成
│   ├── steam/              # Steam 接口集成（与 bangumi 平级）
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

## 二、配置 (`src/config.rs`)

**文件**: [src/config.rs](src/config.rs)

**功能**: 加载和管理应用配置。

### 核心结构

- **`Config`** — 顶层配置
  - `root: PathBuf` — 配置文件所在目录（即 `database/`）
  - `locale: String` — 系统语言环境（自动检测）
  - `anime: ConfigAnime` — 番剧相关配置
  - `game: ConfigGame` — 游戏相关配置
- **`ConfigAnime`** — 番剧配置
  - `path: PathBuf` — 番剧数据相对于 root 的路径
  - `bangumi_api: String` — Bangumi API 地址（默认 `api.bgm.tv`）
- **`ConfigGame`** — 游戏配置
  - `path: PathBuf` — 游戏数据相对于 root 的路径
  - `steam: ConfigSteam` — Steam 接口配置
- **`ConfigSteam`** — Steam 配置
  - `api_key: String` — Web API 密钥，留空则只有商店接口可用
  - `steam_id: Option<String>` — SteamID64，拉玩家数据（已拥有游戏等）时需要
  - `language` / `country` — 商店接口的文案语言与价格分区（默认 `schinese` / `cn`）
  - `api` / `store_api` — 两个 API 的 base 地址
- **`CONFIG: LazyLock<Config>`** — 全局单例，首次访问时加载

### 配置加载流程

1. 在 debug 模式下，自动切换工作目录到 `database/`
2. 读取当前目录的 `aki.toml`
3. 解析 TOML，设置 `root` 和 `locale`
4. 通过 `anime_data_path()` 获取番剧 JSON 文件的存储路径

### 路径方法

`Config` 集中提供各数据路径，均用 `OnceLock` 缓存：

| 方法 | 路径 |
|------|------|
| `anime_data_path()` | `{root}/{anime.path}` |
| `anime_fjall_index_path()` | `{anime_data_path}/.aki/fjall-index`（索引数据库目录） |
| `anime_state_db_path()` | `{root}/anime-state.db`（`export-state` 的默认输出） |
| `bangumi_subject_path()` | `{root}/bangumi/subject` |
| `game_data_path()` | `{root}/{game.path}` |
| `steam_data_path()` | `{root}/steam`（Steam 原始响应） |
| `namespace_data_path(kind)` | 按命名空间取目录，未登记的退回 `{root}/{kind}` |

`namespace_data_path()` 是 [`MetaId`](src/meta/meta_id.rs) 找数据目录的入口：`MetaId::json_data_path()` 调用 `T::json_path(id)`，而 `Namespace::json_path` → `Namespace::data_path()`，所以路径规则完全由命名空间决定，`meta` 模块里不再有任何领域名字的硬编码。

### 配置文件格式 (`database/aki.toml`)

```toml
[anime]
path = "./anime"
bangumi_api = "https://proxy.scalt.xyz/api.bgm.tv"

[game]
path = "./game"

[game.steam]
api_key = "<Steam Web API key>"
# steam_id = "7656119xxxxxxxxxx"
language = "schinese"
country = "cn"
```

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
}
```

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
pub async fn add_with_bangumi(client: &reqwest::Client, bangumi_id: i64)
    -> Result<Anime, AddWithBangumiError>
```

流程：
1. 调用 Bangumi API 获取条目数据
2. 将 `BangumiSubject` 转换为 `Anime`
3. 保存为 JSON 文件

### 3.7 命令：update_with_bangumi (`src/anime/command/`)

**文件**: [src/anime/command/update_with_bangumi.rs](src/anime/command/update_with_bangumi.rs)

**功能**: 按 Bangumi ID 找到已有番剧，重新拉取条目数据并刷新其中的 Bangumi 数据。

```rust
pub async fn update_with_bangumi(client: &reqwest::Client, bangumi_id: i64)
    -> Result<Anime, UpdateWithBangumiError>
```

流程：
1. 载入 `.aki/fjall-index/` 里的 bangumi-id 索引，按 `bangumi_id` 直接定位番剧；索引未命中或索引项已失效（文件被改名、删除）时回退到遍历数据目录，并把结果写回索引：一条都没有报 `NotFound`，多于一条报 `Ambiguous`（先查再联网，失败时不会写入任何数据）
2. 调用 Bangumi API 获取条目数据并保存
3. 用最新条目刷新该番剧
4. 写入数据文件并同步索引；若 `meta.id` 变化导致改名，先把索引指向新文件，再删除旧文件，新文件名已被其他条目占用时报 `TargetExists`（不覆盖）

刷新规则：

| 字段 | 处理 |
|------|------|
| `info` | 整体替换为 Bangumi 最新数据 |
| `i18n` | 按语言合并：Bangumi 能提供的语言整体覆盖，其余语言保留 |
| `meta.id` | 按最新「[上映日期] 原名」重算，数据文件名随之改变 |
| `meta.data.title` | 刷新为最新 `name_cn` |
| `meta.data.create_time` | 保留原值 |
| `state` | 用户填写，原样保留 |

> `search` 走 `.aki/fjall-index/` 的 `title` keyspace；索引不存在或还没建时可以退回扫数据目录（只读标题），索引只影响快慢，不影响结果。

### 3.8 索引 (`src/anime/index/`)

**目录**: [src/anime/index/](src/anime/index/)

索引就是 `{anime_data_path}/.aki/fjall-index/` 下的一个 fjall 数据库，随时可以删除并从数据目录重建。里面有**两个平表 keyspace**，每个都是「键 → 一个 `meta.id`」，没有倒排、没有分词：

| keyspace | 键 | 值 | 用途 |
|----------|----|----|------|
| `bangumi-id-to-meta-id` | `bangumi_id`（`i64::to_be_bytes`） | `meta.id` | `update-with-bangumi` 直接定位数据文件 |
| `title-to-meta-id` | 小写化的标题 | `meta.id` | `search` 按标题找番剧 |

整个模块只有 `fjall.rs` 一个文件（[src/anime/index/fjall.rs](src/anime/index/fjall.rs)）。

```rust
pub struct AnimeIndex { /* Database + 两个 Keyspace */ }
impl AnimeIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, fjall::Error>;
    pub fn open_or_warn() -> Option<Self>;              // 打不开就打印警告并退化成「没有索引」
    pub fn set_bangumi_id_to_meta_id(&self, bangumi_id: i64, meta_id: &str) -> Result<(), fjall::Error>;
    pub fn get_meta_id_of_bangumi_id(&self, bangumi_id: i64) -> Option<String>;
    pub fn set_title_to_meta_id(&self, title: &str, meta_id: &str) -> Result<(), fjall::Error>;
    pub fn set_anime_index(&self, anime: &Anime) -> Result<(), Vec<(&'static str, fjall::Error)>>;
    pub fn set_anime_index_or_warn(&self, anime: &Anime);   // 逐个 keyspace 打印失败原因
    pub fn title_entries(&self) -> impl Iterator<Item = (String, String)>;  // (小写标题, meta.id)
    pub fn build() -> Result<BuildResult, fjall::Error>;
    pub fn flush / flush_or_warn;                       // 批量写入后落盘
}

pub struct BuildResult {
    pub bangumi_id_to_meta_id: usize,
    pub title_to_meta_id: usize,
    pub errors: Vec<fjall::Error>,   // 单条写入失败不会中断整体重建
}
```

- 键值都按 UTF-8 存（`meta.id` 原样、`bangumi_id` 用大端字节），取出即可读、不依赖 serde
- 一条番剧有几个标题（**原名 + 各语言翻译**）就在 `title-to-meta-id` 里占几个键，见 [`Anime::all_titles`]；标题键统一小写、查询也小写化，因此大小写不敏感。空标题会被 `set_title_to_meta_id` 跳过——fjall 不接受空键
- 打开数据库会**独占目录**，同一目录在同一进程里只应存在一个 `AnimeIndex` 实例，因此各命令都是用完即弃

**写入时机**：`add-with-bangumi` 保存新番剧后、`update-with-bangumi` 遍历找到番剧后，以及更新导致 `meta.id` 重算后（都走 `set_anime_index_or_warn` + `flush_or_warn`）。

**重建**：`AnimeIndex::build()` 遍历数据目录，先清空两个 keyspace 再整体写入（已删除的番剧不会残留在索引里），单条失败记进 `BuildResult::errors` 而不中断，最后落一次盘；`aki-cli anime build-index` 就是调它，打开/清空/落盘失败会带着错误返回而不是只打印警告。**注意**：`update-with-bangumi` 改了 `meta.id` 后旧键不会自动消失，想让索引立刻干净就跑一次 `build-index`。

**读取方式**：命中的索引项仍要能读到对应文件、且其 `bangumi_id` 与查询一致才算有效，否则回退到遍历并修复索引。

**容错**：这是可重建的缓存。各命令用 `open_or_warn()` 拿索引，拿不到就是 `None`，定位退化成遍历数据目录、写入被跳过，只打印一行警告——都不会让真正的增删改失败。

#### 搜索 (`src/anime/command/search.rs`)

**文件**: [src/anime/command/search.rs](src/anime/command/search.rs)

**功能**: 按标题匹配字符串，[`Anime::all_titles`]（原名 + 各语言翻译标题）全部参与匹配，大小写不敏感。

- 先扫 `title-to-meta-id` keyspace（约 1800 条键值对，一次全表扫描几十微秒），只对最终要返回的那几条去读数据文件
- 索引目录不存在、打不开、或者标题表是空的（没跑过 `build-index`）时，退回扫数据目录；扫文件时只反序列化 `meta.id` 与标题（`TitleRecord`），不解析整个 `Anime`——完整反序列化在 debug 构建下要慢两个数量级
- 分档：完全相同（4.0）> 前缀（3.0）> 包含（2.0），同档内标题越短越靠前（`rank - 字数/1000`）；一条番剧取所有标题里的最高分，按分数降序截取 `limit` 条
- 空查询直接返回空结果

> 实测（621 条番剧 / 1808 个标题）：`search 物语` 走索引 debug 11~17 ms、release 60 ms（release 里进程启动占大头），索引目录 0.6 MB。之前「遍历 + 完整反序列化」的写法是 4.6 s。


### 3.9 命令：export_state / import_state (`src/anime/command/`)

**文件**: [export_state.rs](src/anime/command/export_state.rs)、[import_state.rs](src/anime/command/import_state.rs)

**功能**: 把全部番剧的 `state`（用户手填的观看状态、评分、评论、日期）导出成一个 SQLite 文件，供 SQL 查询或其他工具消费；也可以反过来从该文件读回并更新数据文件。

```rust
pub fn export_state(output: Option<&Path>) -> Result<(PathBuf, usize), ExportStateError>
pub fn import_state(input: Option<&Path>, dry_run: bool) -> Result<ImportStateResult, ImportStateError>
```

- 默认输出/读取 [`Config::anime_state_db_path()`](src/config.rs)（即 `{root}/anime-state.db`），可用 `--output` / `--input` 覆盖
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

流程：请求 `store.steampowered.com/api/appdetails?appids={appid}&l={language}&cc={country}` → 原始响应写到 `database/steam/appdetails{appid}.json` → 解析成 `AppDetails` → 转成 `Game` → 写到 `database/game/[日期] 名称.json`。

> 文件名里的 `/` 会被 `sanitize-filename` 去掉，所以「存放名 `appdetails/{appid}`」落到磁盘是 `appdetails{appid}.json`。要真正分子目录的话，把存放名换成不含 `/` 的分段、或让 `Namespace::json_path` 支持多级路径即可。

### 3.5.6 命令：sync-owned-games (`src/game/command/sync_owned_games.rs`)

```rust
pub async fn sync_owned_games(client: &reqwest::Client, create_missing: bool)
    -> Result<SyncResult, SyncOwnedGamesError>
```

流程：

1. 调 `IPlayerService/GetOwnedGames`（用配置里的 `steam_id` + `api_key`），整份响应原文写到 `database/steam/owned-games.json`
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

### 3.6.3 原始响应落盘 (`src/steam/store.rs`)

`SteamNamespace` 把 `database/steam` 注册成一个命名空间，于是原始数据可以复用同一套路径规则：

```rust
pub struct SteamRawStore<T> { pub url: String, pub raw: String, /* marker */ }

impl<T> SteamRawStore<T> {
    pub fn save_named(&self, name: &str) -> Result<PathBuf, io::Error>;  // database/steam/{name}.json
    pub fn load_named(name: &str) -> Result<String, io::Error>;
    pub fn load_path(path) -> Result<String, io::Error>;
}
```

`appdetails` 的存放名是 `appdetails{appid}`（`/` 被文件名安全化去掉），因此文件落在 `database/steam/appdetails{appid}.json`。里面存的是**响应原文**——字段顺序、缩进、外壳都保持原样，`serde_json` 只用于读取，不参与写盘；和 Bangumi 存 `Subject` 原样响应同理：字段解析错了可以离线重来，补字段也不用重新打接口。

### 3.6.4 脏日期解析 (`src/steam/date.rs`)

- `detect_year()`：任意一个落在 [1900, 2100] 的 4 位数
- `detect_month()`：英文月名（简写/全称，大小写不敏感）
- `detect_numeric_month()` / `detect_quarter()`：中文 `7 月` / `第 1 季度`，以及 `Q1`~`Q4`
- `parse_date()` / `parse_steam_date()`：只有「年 + 月 + 日」都拿得到才返回 `Some`；`"2024"`、`"Jan 2024"`、`"第 1 季度"` 一律 `None`，交给 `GameRelease` 用别的粒度表示
- 已覆盖中英文两种写法的单元测试，包括「不该被误判成某一天」的反例

---

## 四、Bangumi API 集成 (`src/bangumi/`)

**目录**: [src/bangumi/](src/bangumi/)

**功能**: 封装 Bangumi API v0 的条目查询。

### 4.1 Subject (`src/bangumi/subject.rs`)

**文件**: [src/bangumi/subject.rs](src/bangumi/subject.rs)

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
- `request_with_id(client, id)` — 异步请求 Bangumi API，获取指定 ID 的条目
- `r#type_enumed()` — 将数字类型转为 `Type` 枚举

**示例数据**: `subject-example.json` 包含一条完整的 Bangumi API 响应样本，用于测试。

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
    fn kind() -> &'static str;                 // 如 "anime"
    fn data_path() -> PathBuf;                 // 如 database/anime，默认走 CONFIG
    fn json_path(name: &str) -> PathBuf;       // {data_path}/{name}.json
    fn save_json(name: &str, bytes: &[u8]) -> Result<PathBuf, io::Error>;
}
```

这是「路径由领域自己决定」的落点：`MetaId::json_data_path()` 只调 `T::json_path()`，`meta` 模块里不再出现 `anime` 之类的具体名字。新增一个领域（或像 `steam` 这样的原始数据目录）只需要实现 `data_path()`：由配置决定的走 `CONFIG.namespace_data_path(kind)`，特殊的直接指定目录（`AnimeNamespace`、`GameNamespace`、`SteamNamespace` 都覆盖了它）。

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

---

## 九、可执行入口 (`src/bin/`)

### 9.1 aki-cli — 主 CLI (`src/bin/cli/`)

**目录**: [src/bin/cli/](src/bin/cli/)

**功能**: 面向用户的主命令行界面，使用 `clap` 构建。

**命令结构**:
```
aki-cli
├── anime (alias: a)
│   ├── add-with-bangumi <bangumi_id>
│   ├── update-with-bangumi <bangumi_id>
│   ├── build-index
│   ├── search <query> [-l <limit>]
│   ├── export-state [-o <path>]
│   ├── import-state [-i <path>] [--dry-run]
│   └── verify
└── game (alias: g)
    ├── add-with-steam <appid>
    ├── sync-owned-games [--no-create] [--all]
    └── verify
```

**文件**:
- [mod.rs](src/bin/cli/mod.rs) — CLI 入口，定义 `SubCommands::Anime` / `SubCommands::Game` 子命令
- [anime.rs](src/bin/cli/anime.rs) — Anime 子命令，`AddWithBangumi` / `UpdateWithBangumi` 接收 Bangumi ID，创建 tokio runtime 调用对应的异步函数，成功后打印番剧标题；`BuildIndex` 重建 fjall 索引并打印写入的 bangumi-id 与标题键数量；`Search` 按标题匹配字符串（走 title 索引，没索引时自动退回扫文件）；`ExportState` / `ImportState` 与 SQLite 互导 `state`
- [game.rs](src/bin/cli/game.rs) — Game 子命令，`AddWithSteam` 接收 Steam appid，创建 tokio runtime 调用 `game_command::add_with_steam`，成功后打印游戏标题与时长/成就；`Verify` 尚未实现

**使用示例**:
```bash
aki-cli anime add-with-bangumi 475354
aki-cli anime update-with-bangumi 475354
aki-cli anime build-index                    # 重建 .aki/fjall-index/ 里的 bangumi-id 索引
aki-cli anime search 物语 -l 5               # 按标题直接匹配，不需要先 build-index
aki-cli anime export-state                  # 写到 database/anime-state.db
aki-cli anime export-state -o /tmp/state.db # 或指定路径
aki-cli anime import-state --dry-run        # 先看会改哪些
aki-cli anime import-state                  # 真写回数据文件
aki-cli game add-with-steam 570             # 拉取 Dota 2 并存原始响应
aki-cli game sync-owned-games --no-create   # 只把「已拥有游戏」里为空的时长/最后游玩补上
aki-cli game sync-owned-games --all         # 顺便给缺失的游戏建条目，并逐条打印
# 或
aki-cli a add-with-bangumi 475354
aki-cli a export-state
aki-cli g add-with-steam 570
aki-cli g sync-owned-games
```

> `game add-with-steam` 需要能访问 `store.steampowered.com`；如果本机走代理，`reqwest` 会读 `HTTPS_PROXY`，没设置代理且直连不通时会报 `http request error`。

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
    ├─ anime_command::add_with_bangumi(bangumi_id)
    │   │
    │   ├── Bangumi API ◄── bangumi::Subject::request_with_id()
    │   │   返回 JSON → 反序列化为 Subject
    │   │
    │   ├── Anime::from(subject)   ─── 自动语言检测 + 国际化填充
    │   │
    │   └── anime.save_as_meta_id()
    │        │
    │        └── MetaId::json_data_path() → T::json_path(id)
    │             → AnimeNamespace::data_path() = CONFIG.anime_data_path()
    │             → database/anime/[YYYY-MM-DD] 番名.json
    │
    └─ game_command::add_with_steam(appid)
        │
        ├── steam::request(AppDetailsEndpoint) ◄── store.steampowered.com/api/appdetails
        │   │
        │   ├── SteamRawStore::save_named("appdetails/{appid}")
        │   │    → database/steam/appdetails{appid}.json（响应原文先落盘）
        │   │
        │   └── AppDetailsEndpoint::parse() → AppDetails
        │
        ├── Game::from(app_details)   ─── 发售日期粒度降级 + 语言检测 + 国际化填充
        │
        └── game.save_as_meta_id()
             │
             └── MetaId::json_data_path() → T::json_path(id)
                  → GameNamespace::data_path() = CONFIG.game_data_path()
                  → database/game/[日期前缀] 游戏名.json
```

两个领域的落盘路径都只经过 `Namespace` 提供的静态方法，因此 `meta` 模块与具体领域完全解耦；`steam` 原始响应只是多注册了一个 `SteamNamespace`。

## 十一、依赖说明

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
