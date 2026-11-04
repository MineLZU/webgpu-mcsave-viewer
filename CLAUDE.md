# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 文档规则（摘自 `AGENTS.md`，两处同步维护）

- 根 `README.md` 属于仓库所有者，未经明确许可不改。要补内容写进 `docs/`，并在回复里说明写了哪个文件。
- 说明性文档只放 `docs/`，`docs/README.md` 是索引。不在根目录或源码目录新建 `.md`。
- 一篇文档答一个问题，一屏读完。不贴源码，用 `路径:行号` 引用。表格优于列表，列表优于段落。中文正文。
- 不写「本次」「刚刚修好」之类会过期的叙述，写当前状态和原因。
- 改了行为、路径、命令、依赖或公共 API，同步改对应 `docs/` 文件。新子系统在 `docs/architecture.md` 分层表加一行。
- 验证结果（命令、退出码、耗时、测试数、日期）只写进 `docs/build.md`。
- 与上游的差异写进 `docs/upstream.md`（尚不存在，首次需要时新建并加入索引）或主题文档的「已知偏差」一节。

## 检查

- 改完 Rust 代码必须通过 `cargo fmt --all -- --check` 和 `cargo clippy -- -D warnings`。完整流程用 `/rust-check`。
- 工具链固定为 `rust-toolchain.toml` 里的日期版 nightly，不要改成浮动 `nightly`。
- CI 是 `.github/workflows/rust.yml`：ubuntu 上只跑 `cargo build` 与 `cargo test`，不跑 fmt/clippy，也不拉 submodule；这两项仍靠本地。`rust-toolchain.toml` 注释提到的 `rust-check.yml` 不存在。
- 升级 wgpu 时同步更新 `wgpu-mc/src/lib.rs:963` 的 `get_backend_description`（见 `Cargo.toml` 注释）。
- `minecraft-assets` 是 edition 2021 的 vendored 上游，其余成员是 2024；它加过一个 `shade` 字段，改动视为与上游的偏差。

## 测试

- `modtree` 的测试从 Neolectrum mod checkout 读文件，用 `WGPU_MC_MOD_DIR` 指定路径；找不到时测试跳过而不是失败，用 `-- --nocapture` 看跳过信息。
- `modtree/src/lib.rs` 的 `workspace_root`/`repo_root` 仍假设工作区在 `rust/` 子目录下，而这里工作区就是仓库根，默认候选路径会多上一层。
- `mesher_bench` 需要 GPU 并读取 `resources/assets`：`cargo run -p wgpu-mc --release --example mesher_bench -- --sections 64 --repeats 5`。

## `minecraft-protocol/`

- 这是我们维护的 Kotlin Multiplatform 协议库 fork，以 submodule 登记在本仓库（`.gitmodules`），`viewer-gateway` 通过 `includeBuild` 引用它。
- 可以修改；在它自己的 git 里提交。它的 `AGENTS.md` 与本文件冲突时，以本文件为准。
- D: 盘是 exFAT，不支持硬链接：`world-io` 的 `WorldDirectoryLockRegressionTest` 硬链接用例在此必然失败，需克隆到 NTFS 盘（`git clone --no-hardlinks`）复核。

## 提交

- 用 Conventional Commits，例如 `fix(world-io): ...`、`docs(viewer): ...`。
