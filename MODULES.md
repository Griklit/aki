# aki — 项目模块文档

## 项目概述

`aki` 是一个用 Rust 编写的个人追番管理工具。核心功能是：
- 以 JSON 文件存储番剧数据（元信息、观看状态、评分）
- 从 [Bangumi](https://bgm.tv) 拉取番剧元数据
- 通过 CLI 命令行交互管理番剧列表

技术栈：Rust 2024 edition, tokio (async), reqwest (HTTP), serde/serde_json (序列化), clap (CLI), tantivy (全文搜索，已引入).

---

## 目录结构

```
aki/
├── Cargo.toml              # Rust 项目配置，定义库+3个二进制目标
├── database/               # 数据存储根目录
│   ├── aki.toml            # 运行时配置文件
│   └── anime/              # 番剧 JSON 数据文件
│       └── [YYYY-MM-DD] 番名.json
└── src/
    ├── lib.rs              # 库根：模块声明 + 公共 API 重导出
    ├── config.rs           # 配置加载模块
    ├── anime/              # 番剧领域模型（核心）
    ├── bangumi/            # Bangumi API 集成
    ├── meta/               # 元数据/ID 管理
    ├── common/             # 共享基础类型
    ├── resource/           # 资源管理（封面等，WIP）
    ├── utils/              # 工具函数
    └── bin/                # 可执行入口
        ├── cli/            # 主 CLI 程序 (aki-cli)
        ├── scripts/        # 一次性数据迁移脚本
        └── test.rs         # 本地环境测试
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
| `BangumiSubject` | `bangumi::subject` | Bangumi 条目 |
| `BangumiSubjectType` | `bangumi::subject` | Bangumi 条目类型 |
| `Language` | `common::language` | 语言枚举 |
| `Score` | `common::score` | 评分 trait |
| `CONFIG` | `config` | 全局配置（懒加载） |
| `Meta` | `meta::core` | 元数据容器 |
| `MetaData` | `meta::core` | 元数据信息 |
| `MetaId` | `meta::core` | 带命名空间的 ID |
| `Namespace` | `meta::namespace` | 命名空间 trait |
| `ImageType` | `resource::image_type` | 图片类型枚举 |
| `Resource` | `resource::core` | 通用资源 |
| `resource_type` | `resource::resource_type` | 资源类型模块 |

---

## 二、配置 (`src/config.rs`)

**文件**: [src/config.rs](src/config.rs)

**功能**: 加载和管理应用配置。

### 核心结构

- **`Config`** — 顶层配置
  - `root: PathBuf` — 配置文件所在目录（即 `database/`）
  - `locale: String` — 系统语言环境（自动检测）
  - `anime: ConfigAnime` — 番剧相关配置
- **`ConfigAnime`** — 番剧配置
  - `path: PathBuf` — 番剧数据相对于 root 的路径
  - `bangumi_api: String` — Bangumi API 地址（默认 `api.bgm.tv`）
- **`CONFIG: LazyLock<Config>`** — 全局单例，首次访问时加载

### 配置加载流程

1. 在 debug 模式下，自动切换工作目录到 `database/`
2. 读取当前目录的 `aki.toml`
3. 解析 TOML，设置 `root` 和 `locale`
4. 通过 `anime_data_path()` 获取番剧 JSON 文件的存储路径

### 配置文件格式 (`database/aki.toml`)

```toml
[anime]
path = "./anime"
bangumi_api = "https://proxy.scalt.xyz/api.bgm.tv"
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
1. 用 `Anime::iter_all()` 找出 `bangumi_id` 匹配的番剧：一条都没有报 `NotFound`，多于一条报 `Ambiguous`（先查再联网，失败时不会写入任何数据）
2. 调用 Bangumi API 获取条目数据并保存
3. 用最新条目刷新该番剧
4. 写入数据文件；若 `meta.id` 变化导致改名，写入新文件后再删除旧文件，新文件名已被其他条目占用时报 `TargetExists`（不覆盖）

刷新规则：

| 字段 | 处理 |
|------|------|
| `info` | 整体替换为 Bangumi 最新数据 |
| `i18n` | 按语言合并：Bangumi 能提供的语言整体覆盖，其余语言保留 |
| `meta.id` | 按最新「[上映日期] 原名」重算，数据文件名随之改变 |
| `meta.data.title` | 刷新为最新 `name_cn` |
| `meta.data.create_time` | 保留原值 |
| `state` | 用户填写，原样保留 |

> `meta.id` 同时是搜索索引的键，改名后需要重新执行 `aki-cli anime build-index`，否则 `search` 会加载不到旧文件。

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

### 5.1 Meta / MetaId / MetaData (`src/meta/core.rs`)

**文件**: [src/meta/core.rs](src/meta/core.rs)

**功能**: 元数据的核心结构。

- **`MetaId<T: Namespace>`** — 带命名空间的唯一标识符
  - `id: String` — UUID v7 或自定义字符串（如 `[2010-10-04] 侵略!イカ娘`）
  - `namespace: T` — 命名空间标记
  - `json_data_path()` — 计算 JSON 文件的存储路径: `<data_path>/<sanitized_id>.json`

- **`MetaData`** — 元数据信息
  - `title: Option<String>` — 可读标题
  - `create_time: Option<DateTime<Utc>>` — 创建时间

- **`Meta<T: Namespace>`** — 元数据容器
  - `id: MetaId<T>` — 标识符
  - `data: MetaData` — 元数据信息
  - 工厂方法: `new(title)` 自动生成 UUID, `with_id(id, title)` 使用自定义 ID

### 5.2 Namespace trait (`src/meta/namespace.rs`)

**文件**: [src/meta/namespace.rs](src/meta/namespace.rs)

**功能**: 定义命名空间抽象。

```rust
pub trait Namespace {
    fn new() -> Self;
    fn kind() -> &'static str;  // 如 "anime"
}
```

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
└── anime (alias: a)
    ├── add-with-bangumi <bangumi_id>
    ├── update-with-bangumi <bangumi_id>
    ├── build-index
    ├── search <query> [-l <limit>]
    └── verify
```

**文件**:
- [mod.rs](src/bin/cli/mod.rs) — CLI 入口，定义 `SubCommands::Anime` 子命令
- [anime.rs](src/bin/cli/anime.rs) — Anime 子命令，`AddWithBangumi` / `UpdateWithBangumi` 接收 Bangumi ID，创建 tokio runtime 调用对应的异步函数，成功后打印番剧标题

**使用示例**:
```bash
aki-cli anime add-with-bangumi 475354
aki-cli anime update-with-bangumi 475354
# 或
aki-cli a add-with-bangumi 475354
aki-cli a update-with-bangumi 475354
```

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
    ▼
anime_command::add_with_bangumi(bangumi_id)
    │
    ├── Bangumi API ◄── bangumi::Subject::request_with_id()
    │   返回 JSON → 反序列化为 Subject
    │
    ├── Anime::from(subject)   ─── 自动语言检测 + 国际化填充
    │
    └── anime.save_as_meta_id()
         │
         └── MetaId::json_data_path()
              → database/anime/[YYYY-MM-DD] 番名.json
              → serde_json 序列化写入
```

## 十一、依赖说明

| 依赖 | 用途 |
|------|------|
| `serde` / `serde_json` | JSON 序列化/反序列化 |
| `clap` | CLI 参数解析 |
| `reqwest` | HTTP 客户端（调用 Bangumi API） |
| `tokio` | 异步运行时 |
| `chrono` | 日期时间处理 |
| `uuid` (v7) | 唯一标识符生成 |
| `sanitize-filename` | 文件名安全化 |
| `toml` | 配置文件解析 |
| `anyhow` / `thiserror` | 错误处理 |
| `sys-locale` | 系统语言环境检测 |
| `tantivy` | 全文搜索（已引入，尚未使用） |
