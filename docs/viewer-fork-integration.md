# 如何复用 minecraft-protocol fork

## checkout 基线

| 项目 | 当前事实 |
|---|---|
| 仓库 | `origin`：`NTS-Official/minecraft-protocol`；`upstream`：`czp3009/minecraft-protocol` |
| 基准 | 上游基线 `cbdf620`；展示器定制为 `883ad48`，已推送到 `origin/master`，见下方「展示器定制」 |
| 管理方式 | `minecraft-protocol/` 以 submodule 登记（`.gitmodules`），主仓库固定其 revision |
| 工程性质 | Kotlin Multiplatform，已有模型、序列化、传输、会话和存档模块，不存在原稿所称的 Rust `codec-only` feature |
| 版本选择 | 26.3：`minecraft-protocol/buildSrc/src/main/kotlin/com/hiczp/minecraft/buildlogic/MinecraftTarget.kt:16` |
| 构建 Java | 25：`minecraft-protocol/buildSrc/src/main/kotlin/com/hiczp/minecraft/buildlogic/BuildVersions.kt:11`；不改变游戏端 Java 21 要求 |

## 复用边界

| 能力 | 集成方式 |
|---|---|
| `nbt`、`nbt-serialization`、`world-format`、`world-io` | 网关按需依赖；复用 NBT、Anvil、压缩和只读文件访问，1.21.11 语义投影由展示器适配层负责 |
| `distribution-metadata` | 用于明确版本的官方资产定位与下载；不使用“最新版本”替代展示器目标。fork 只建模 26.3 的版本元数据格式，1.21.11 缺 `arguments.default-user-jvm`，`viewer-gateway/assets-tool/src/main/kotlin/com/nts/viewer/assets/OfficialFiles.kt:41` 只读原始 JSON 的 `downloads`，再用 fork 的 `MinecraftDownload` 解析 |
| Native HTTP/WebSocket | 参考 `minecraft-protocol/demo/web-map/build.gradle.kts:24` 与 `minecraft-protocol/demo/web-map/src/serverMain/kotlin/com/hiczp/minecraft/demo/webmap/WebMapServer.kt:57`；新建网关应用，不扩写二维地图产品 |
| 注册表与维度 | 从 1.21.11 服务端捕获并附内容哈希；不使用 fork 当前生成的 vanilla 默认值 |
| 原版包模型与会话 | 不加入热流主链；展示协议单独版本化，Rust 消费展示协议，不直接链接 Kotlin/Wasm 对象 |
| 依赖固定 | submodule 固定 fork revision；`viewer-gateway/settings.gradle.kts:24` 以 `includeBuild` 接入，并复用 fork 的版本目录 |

## 展示器定制

| 改动 | 入口与语义 |
|---|---|
| 方块状态 NBT 布局 | `minecraft-protocol/world-format/src/commonMain/kotlin/com/hiczp/minecraft/world/format/BlockStateNbtCodec.kt:15`：`BlockStateNbtLayout`；默认 `ID_PROPERTIES` 保持 26.x 行为，`NAME_PROPERTIES` 显式读写 1.21.11 `Name/Properties`，不接受裸字符串 ID |
| 调用方式 | `BlockStateNbtDecoder(BlockStateNbtLayout.NAME_PROPERTIES) { blockId -> ... }` 放入 `ChunkNbtDecoderContext`；不自动探测布局 |

## 已知偏差与阻塞点

| 差异 | 必须采用的处理 |
|---|---|
| NBT 布局只改字段名 | `BlockStateNbtLayout.NAME_PROPERTIES` 只读写 `Name/Properties`；方块 ID、属性值和默认状态仍须由 1.21.11 字典提供，其余 Chunk 字段未做版本迁移 |
| 调色板不能照原稿回退 | `minecraft-protocol/protocol-serialization/src/commonMain/kotlin/com/hiczp/minecraft/protocol/serialization/internal/PalettedContainerCodec.kt:36` 根据注册表大小计算 direct 位宽；不把 15/17-bit 写成统一阈值，不混用磁盘 NBT 与网络容器布局 |
| 生成器不是复制报告即可换版本 | `minecraft-protocol/buildSrc/src/main/kotlin/com/hiczp/minecraft/buildlogic/OfficialMinecraftPacketAnalysisTask.kt:21` 分析官方类；1.21.11 混淆映射、类布局和生成输入均需独立适配，完整包生成不在主线 |
| Native 不等于无需构建 JDK | fork 构建仍受 Java 工具链和生成任务约束；Native 成品运行不依赖 JVM，不将整个 fork 降级为 Java 21 |
| 通用容器不保证旧存档语义 | 使用底层 NBT 入口读取所需字段；不能仅修键名就宣称完整支持旧版 `level.dat`、实体或数据包 |

验收需要真实 1.21.11 存档、显式版本上下文、无 26.x 默认值污染，以及 Native 链接和运行检查。仅发现构建目标配置不代表已通过验证；冷读边界见[冷区块](viewer-cold-chunks.md)。
