# 工程分层：已有代码与拟建组件

## 已有代码

| 层 | 职责与入口 |
|---|---|
| Rust workspace | `Cargo.toml:8`：成员为 `wgpu-mc`、`minecraft-assets`、`modtree` |
| 渲染器 | `wgpu-mc/src/lib.rs:115`：`WmRenderer`；`wgpu-mc/src/mc/chunk.rs:1395`：Section 烘焙 |
| 资源模型 | `minecraft-assets/src/lib.rs:1`：资源描述与模型类型；不等于已准备好的 1.21.11 资产 |
| 测试辅助 | `modtree/src/lib.rs:1`：定位外部模组 checkout；不能替代浏览器运行入口 |
| 协议库 checkout | `minecraft-protocol/settings.gradle.kts:1`：独立 Gradle/KMP 工程，集成约束见 [fork 边界](viewer-fork-integration.md) |

## 拟建组件

以下是实现阶段的目录约定，目录和接口均尚未创建；不将 Kotlin 工程伪装为 Cargo crate。

| 拟建目录 | 职责与依赖 |
|---|---|
| `server-bridge/` | 独立 Java 21 Gradle 工程；共享捕获输出契约，Fabric 入口先行，NeoForge 后续实现 |
| `viewer-gateway/` | 独立 KMP 应用构建，产出 Native 网关；通过 Gradle composite build 引用固定 revision 的 fork 所需模块 |
| `crates/viewer-protocol/` | 展示协议 Rust 解码和共用样本校验；不依赖原版 Minecraft 包会话 |
| `crates/viewer-world-state/` | Section 状态、版本、失效与烘焙调度；向渲染器提供 `BlockStateProvider` |
| `crates/viewer-web/` | WASM 入口、WebGPU 生命周期和 `wgpu-mc` 适配；CPU 烘焙与 GPU 提交职责分离 |
| `web-client/` | Vite/TypeScript 宿主、WebSocket、资源下载、Canvas 与输入 |

新增组件时将对应行改为实际入口；渲染内核继续维护于 `wgpu-mc/`，避免复制出第二套引擎。阶段顺序见[架构方案](wgpu_mc_1_21_11_web_viewer_architecture-v2.md)。
