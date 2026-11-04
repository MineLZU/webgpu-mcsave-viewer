# 展示协议 v1 的字节布局

一个 WebSocket 二进制帧承载一条消息：首字节为类型，其余字段小端序，帧尾不得有剩余字节。实现：`crates/viewer-protocol/src/codec.rs:18`（Rust）、`viewer-gateway/protocol/src/commonMain/kotlin/com/nts/viewer/protocol/ProtocolCodec.kt:8`（Kotlin，JVM 与 Native）。

## 消息

| 类型 | 方向 | 字段（按顺序） |
|---|---|---|
| `0x01` Hello | 网关→浏览器 | `version u16`=1、`session u64`、`game_version str`、`dictionary_sha256 [32]`、`assets_sha256 [32]`、`block_state_count u32`、`biome_count u32`、`n u8`、n×(`id str`、`min_y i32`、`height u32`) |
| `0x02` Subscribe | 浏览器→网关 | `generation u32`、`dimension u16`、`n u8`、n×(`min_x`、`min_z`、`max_x`、`max_z` 各 `i32`，闭区间)；n=0 即取消 |
| `0x03` SectionSnapshot | 网关→浏览器 | `pos`、`revision u64`、`light_revision u64`、方块容器（4096 格）、biome 容器（64 格）、天空光、方块光 |
| `0x04` BlockDelta | 网关→浏览器 | `pos`、`base_revision u64`、`revision u64`、`n u16`、n×(`index u16`、`state u32`) |
| `0x05` LightUpdate | 网关→浏览器 | `pos`、`light_revision u64`、天空光、方块光 |
| `0x06` ChunkLifecycle | 网关→浏览器 | `dimension u16`、`x i32`、`z i32`、`state u8`：0 热态、1 卸载、2 不可用 |
| `0x07` ResyncRequest | 浏览器→网关 | `kind u8`：0 后接 `generation u32`；1 后接 `pos` |

## 公共编码

| 元素 | 编码 |
|---|---|
| `str` | `u16` 字节长度 + UTF-8 |
| `pos` | `dimension u16`（Hello 中的序号）、`x i32`、`y i32`（Section Y）、`z i32` |
| 格序 | `index = (y×16 + z)×16 + x`；biome 为 4×4×4 立方体，同序 |
| 容器 | `0` + `value u32`（单值）；或 `1` + `palette_len u16` + 值×`u32` + `bits u8` + `ceil(格数×bits/8)` 字节，低位在前、首格在前 |
| 光照层 | `0` 未知；`1` + 2048 字节，每字节两格，低半字节在前 |

## 两层校验

| 层 | 拒绝条件 | 错误类别 |
|---|---|---|
| 编解码 | 截断、尾随字节、未知类型、标签越界、协议版本≠1、调色板长度不在 2..=格数或重复、`bits`≠最小位宽、索引越出调色板、`revision`≤`base_revision`、变更数不在 1..=4096、变更索引越界或重复、维度 `min_y`/`height` 非 16 倍数或越出 −2032..2032、区间最小值大于最大值、维度/区间超过 64 | `truncated`、`trailing_bytes`、`unknown_type`、`invalid` |
| 会话（`SessionBounds`） | 维度序号不存在、Section Y 越出该维度、状态/biome ID ≥ Hello 中的数量 | `out_of_bounds` |

单值容器是均匀容器的唯一合法形式，因此同一内容只有一种编码；两种实现都要求解码再编码得到相同字节。

## 跨语言样本

`crates/viewer-protocol/samples/` 中的 40 个 `.bin` 与 `manifest.json` 是契约，由 `crates/viewer-protocol/tests/samples.rs:1` 生成并逐字节比对；`VIEWER_PROTOCOL_WRITE_SAMPLES=1` 才会重写。Kotlin 侧由 `viewer-gateway/protocol/src/commonTest/kotlin/com/nts/viewer/protocol/SharedSamplesTest.kt:30` 在 JVM 与 `mingwX64` 上读取同一目录。覆盖负坐标、不同维度高度、空段、截断、非法长度、越界状态 ID 与不匹配 revision。

## 已知偏差

| 差异 | 处理 |
|---|---|
| 桥接入口未实现 | 网关只有浏览器 `/viewer`；模组到网关的 `/bridge` 在 P1 随 Fabric 模组实现 |
| 无压缩、无认证 | 首期按计划不压缩；网关默认只监听 `127.0.0.1`，凭据在 P2 前补齐 |
