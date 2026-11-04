# 工程分层：已有代码与拟建组件

## 已有代码

| 层 | 职责与入口 |
|---|---|
| Rust workspace | `Cargo.toml:8`：成员为 `wgpu-mc`、`minecraft-assets`、`modtree` |
| 渲染器 | `wgpu-mc/src/lib.rs:115`：`WmRenderer`；`wgpu-mc/src/mc/chunk.rs:1395`：Section 烘焙 |
| 资源模型 | `minecraft-assets/src/lib.rs:1`：资源描述与模型类型；不等于已准备好的 1.21.11 资产 |
| 测试辅助 | `modtree/src/lib.rs:1`：定位外部模组 checkout；不能替代浏览器运行入口 |
| 协议库 checkout | `minecraft-protocol/settings.gradle.kts:1`：独立 Gradle/KMP 工程，集成约束见 [fork 边界](viewer-fork-integration.md) |
| 展示协议（Rust） | `crates/viewer-protocol/src/lib.rs:1`：v1 编解码、`SessionBounds` 与 `samples/` 共用样本；布局见[展示协议](viewer-protocol.md) |
| 网关工程 | `viewer-gateway/settings.gradle.kts:1`：独立 Gradle 工程，沿用 fork 的版本目录并 `includeBuild` fork；子项目 `protocol`（Kotlin 编解码，JVM＋`mingwX64`）、`assets-tool`（JVM 导出工具）、`app`（Native 网关） |
| 浏览器状态 | `crates/viewer-world-state/src/lib.rs:1`：按会话/维度/坐标保存 Section，校验 revision，维护 27 邻域脏集与 resync 集 |
| 浏览器 wasm | `crates/viewer-web/src/web.rs:1`：WebGPU 设备与画布、消息入口、按哈希加载的字典与资源包；`src/render.rs` 经 `wgpu-mc` 烘焙并绘制地形，见 [WebGPU 适配](viewer-webgpu.md) |
| 浏览器宿主 | `web-client/src/main.ts:1`：Vite/TypeScript，用 npm（exFAT 上 pnpm 无法建符号链接）；`npm run wasm` 构建 `crates/viewer-web` |

## 拟建组件

以下是实现阶段的目录约定，目录和接口均尚未创建；不将 Kotlin 工程伪装为 Cargo crate。

| 拟建目录 | 职责与依赖 |
|---|---|
| `server-bridge/` | 独立 Java 21 Gradle 工程；共享捕获输出契约，Fabric 入口先行，NeoForge 后续实现；P1 起替代网关的合成地形 |

新增组件时将对应行改为实际入口；渲染内核继续维护于 `wgpu-mc/`，避免复制出第二套引擎。阶段顺序见[架构方案](wgpu_mc_1_21_11_web_viewer_architecture-v2.md)。
