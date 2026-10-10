# aki

一个用 Rust 写的个人追番 / 追游戏管理命令行工具。

- 用 JSON 文件保存番剧与游戏数据（元信息、观看/游玩状态、评分、评论）
- 从 [Bangumi](https://bgm.tv) 拉番剧元数据，从 [Steam](https://store.steampowered.com) 拉游戏元数据
- 原始接口响应单独落盘，解析逻辑改了也能离线重跑
- 番剧状态可导出成 SQLite，用 SQL 随便查，也能再导回数据文件

## 环境要求

- Rust（2024 edition，建议用 stable 最新版）
- 无需系统安装 SQLite（`rusqlite` 用 `bundled` 特性）

## 构建

```bash
cargo build            # debug
cargo build --release  # release
```

## 数据目录与配置

数据目录就是放着 `aki.toml` 的那个目录（调试构建默认进 `./aki-data`）。首次使用先建好它：

```
aki-data/
├── aki.toml        # 配置
├── anime/          # 番剧 JSON，文件名形如 [2010-10-04] 侵略!イカ娘.json
├── game/           # 游戏 JSON，文件名形如 [2013-07-09] Dota 2.json
└── source/         # 接口原始响应
    ├── bangumi/subject/{id}.json
    └── steam/appdetails/{appid}.json
```

`aki.toml` 示例（`[bangumi]` / `[steam]` 两段都可整段省略）：

```toml
[anime]
path = "./anime"

[game]
path = "./game"

[bangumi]
path = "./source/bangumi"
bangumi_api = "https://api.bgm.tv"

[steam]
path = "./source/steam"
api_key = "<Steam Web API key>"   # 留空则只有商店接口可用
# steam_id = "7656119xxxxxxxxxx"  # 拉「已拥有游戏」时需要
language = "schinese"
country = "cn"
```

## 常用命令

```bash
# 番剧（别名 a）
aki-cli anime add-with-bangumi <bangumi_id>     # 按 Bangumi ID 添加
aki-cli anime update-with-bangumi <bangumi_id>  # 重新拉取条目，刷新元信息
aki-cli anime search <关键词> [-l <条数>]        # 按标题搜索（原名 + 各语言译名）
aki-cli anime export-state -o <路径>             # 导出观看状态到 SQLite（路径必填）
aki-cli anime import-state -i <路径> [--dry-run] # 从该文件导回（路径必填）
aki-cli anime build-index                        # 重建可删除、可重建的本地索引

# 游戏（别名 g）
aki-cli game add-with-steam <appid>              # 按 Steam appid 添加
aki-cli game sync-owned-games [--no-create] [--all]  # 同步已拥有游戏的时长/最后游玩
```

诊断信息走 `stderr` 并带级别符号前缀（`[#] [+] [?] [!] [x]`），命令结果走 `stdout`。

> `game add-with-steam` 需要能访问 `store.steampowered.com`；走代理时设置 `HTTPS_PROXY` 即可。

## 项目结构

```
src/
├── lib.rs        # 库根，重导出公共 API
├── config/       # aki.toml 的模型与加载
├── anime/        # 番剧领域：core / info / state / score / index / command
├── game/         # 游戏领域，与 anime 同构
├── meta/         # 元数据与带命名空间的 ID
├── source/       # 数据来源：bangumi / steam 的接口与原始响应落盘
├── common/       # 共享基础类型（Language、Score）
├── resource/     # 资源管理（早期 WIP）
├── utils/        # 工具函数
└── bin/          # aki-cli、scripts（一次性迁移）、test
```

## 文档

各模块的详细说明（数据模型、索引、路径归属、日志约定等）见 [MODULES.md](MODULES.md)。
