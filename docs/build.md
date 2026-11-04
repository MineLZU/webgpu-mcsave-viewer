# 验证记录

记录实际执行范围；规划中的运行验收条件见各主题文档，不代表已经通过。

## P0：网关、导出与浏览器验收（2026-11-04）

在 `viewer-gateway/` 内逐个执行，`--max-workers=1`，Java 25（GraalVM 25.1.3）。

| 命令 | 退出码 | 耗时 | 结果 |
|---|---|---|---|
| `./gradlew :protocol:jvmTest` | 0 | 44 s | `SharedSamplesTest` 读 `crates/viewer-protocol/samples/` 40 个样本，解码字段、重编码字节、错误类别与 `manifest.json` 一致 |
| `./gradlew :protocol:mingwX64Test` | 0 | 50 s | 同上，Kotlin/Native |
| `./gradlew :assets-tool:run --args="--version 1.21.11 --out generated/1.21.11"` | 0 | 23 s | 29671 个方块状态、65 个 biome、4771 个资源文件；client/server JAR 的 SHA-1 与版本元数据一致。首次运行因 fork 只建模 26.3 元数据格式（缺 `arguments.default-user-jvm`）失败，改为只读原始 JSON 的 `downloads` |
| `./gradlew :app:linkDebugExecutableMingwX64` | 0 | 40 s | `viewer-gateway.exe` 15.5 MB；首次因 JVM 专有的 `toSortedMap` 编译失败 |
| Native 网关 `GET /content/{sha256}` | — | — | 字典 3193766 B、资源包 1846790 B，下载后 SHA-256 与清单一致；未知哈希 404 |
| Native 网关 `/viewer` WebSocket（Node 脚本） | 0 | 1.6 s | `Hello` 哈希与计数一致；订阅 3×3 区块列收到 9 条 `ChunkLifecycle`、216 个 Section 快照（Y −4..19） |
| Native 网关路径穿越探测 | — | — | 修复前 `/..%2F..%2FCargo.toml` 返回仓库根文件（解码后的单段含 `/`）；修复后该式及 `%2e%2e`、`%5C`、`%3A`、`.` 均 404，`/` 仍 200 |
| Edge 146 无头（`--enable-unsafe-webgpu`），打开 `http://127.0.0.1:8765/` | 0 | 125 s | Intel gen-12lp 适配器；6936 个 Section 收齐并烘焙完、0 待烘焙，3 个状态无模型（三种空气）；控制台无错误。截图 `target/viewer-chromium.png`：1250×578 中 71.2% 为地形 |

浏览器验收用的是网关合成地形（`SyntheticWorld`），不是真实存档或捕获数据；Fabric 捕获属于 P1。

## P0：展示协议与 wasm 编译（2026-11-03）

在仓库根执行，工具链为 `rust-toolchain.toml` 固定的 nightly。

| 命令 | 退出码 | 结果 |
|---|---|---|
| `cargo check -p wgpu-mc --target wasm32-unknown-unknown` | 0 | 修复前 naga `glsl-in` 缺 `apply_default_interpolation`、`wgpu::Texture` 非 `Sync` 两处失败；修复见 `wgpu-mc/Cargo.toml` 的 wasm32 依赖段 |
| `cargo check -p wgpu-mc` | 0 | 原生目标不受影响 |
| `cargo test -p viewer-protocol` | 0 | 2 个测试；40 个样本按 `manifest.json` 解码/拒绝，与重新生成的字节一致 |
| `cargo clippy -p viewer-protocol --all-targets -- -D warnings`、`cargo fmt --all -- --check` | 0 | 无警告、无格式差异 |
| `cargo test -p viewer-world-state` | 0 | 7 个测试：会话外/越界拒绝、增量匹配、基准失配转 resync、旧消息不回滚、27 邻域脏集、独立光照、卸载即未知 |
| `cargo test -p viewer-web` | 0 | 4 个测试：字典顺序与格式校验、资源包 deflate 解包与越界路径拒绝 |
| `cargo clippy -p viewer-web`（原生与 `wasm32-unknown-unknown`）`-- -D warnings` | 0 | 无警告 |
| `cargo test -p viewer-web --test render`（2026-11-04） | 0 | 原生适配器经浏览器同一路径画出 1 个 Section：65536 像素中 19115 为地形、约 7600 为草色；帧存于 `target/viewer-web-render.png`。开背面剔除时绿色仅 6 像素，故关闭剔除，见 `docs/viewer-webgpu.md` 已知偏差 |
| `cargo clippy -p viewer-web`（原生 `--all-targets` 与 wasm32）、`cargo clippy -p wgpu-mc`，均 `-D warnings` | 0 | 接入渲染后无警告；`wgpu-mc` 时钟改用 `web-time` 后原生与 wasm32 均通过 |
| `npm run wasm`、`npx tsc --noEmit`、`npx vite build`（`web-client/`） | 0 | 接入渲染后 wasm 20 MB（gzip 3.75 MB，debug 构建）；`pnpm install` 在 exFAT 上因无法建符号链接失败，改用 npm |

Kotlin 侧在 2026-11-03 未能完成构建（首次因 `assets-tool` 目录未创建失败，第二次因系统可用内存不足被终止）；2026-11-04 的结果见上一节。

## fork 方块状态 NBT 布局（2026-11-03）

在 `minecraft-protocol/` 内执行，Java 25（GraalVM 25.1.3）。

| 命令 | 退出码 | 耗时 | 结果 |
|---|---|---|---|
| `./gradlew :world-format:jvmTest` | 0 | 190 s | 146 个测试，0 失败、0 跳过；其中 `BlockStateNbtTest` 4 个 |
| `./gradlew :protocol-world:jvmTest :world-io:jvmTest :protocol-configuration-vanilla:jvmTest` | 1 | 908 s | `protocol-world` 10 个、`protocol-configuration-vanilla` 27 个全部通过；`world-io` 未运行 |
| `./gradlew :world-io:jvmTest`（D: 盘） | 1 | 327 s | 245 个测试，243 通过、2 失败、0 跳过；失败均为 `WorldDirectoryLockRegressionTest` 的硬链接用例 |
| 同一测试类，fork 克隆到 C: 盘（NTFS），`--tests ...WorldDirectoryLockRegressionTest` | 0 | 42 s | 6 个测试全部通过，含两个硬链接用例 |

| 环境问题 | 现象与处理 |
|---|---|
| D: 盘为 exFAT | 不支持硬链接，`Files.createLink` 报「函数不正确」；`world-io` 的硬链接锁测试需在 NTFS 上运行 |
| HeadlessMC 资源下载 | `:downloadHeadlessMcAssetReplacements` 依赖 `raw.githubusercontent.com`；网络不可达时 `world-io:jvmTest` 无法开始，跳过该任务也不行 |
| Git Bash `curl` | schannel 报 `CRYPT_E_NO_REVOCATION_CHECK`（退出码 35），不代表主机不可达；PowerShell 与 Gradle 不受影响 |

未执行 JS、WasmJS、Native 测试；该改动不涉及压缩，默认布局写出的字节不变。

## 文档静态核对（2026-11-02）

| 检查／命令 | 退出码 | 结果与范围 |
|---|---|---|
| PowerShell 内联文档校验 | 0 | 9 篇文档、19 个本地链接、28 个源码行号引用；目标文件及非空行存在，索引覆盖全部主题 |
| 同一校验中的格式与迁移检查 | 0 | 无行尾空白，均以换行结束，无大段源码；根目录原稿已移除，目标文档位于 `docs/` |
| `git diff --check` | 0 | 已跟踪文件无空白问题；新增且未跟踪的文档由前述内联校验覆盖 |
| `git status --short` | 0 | 只有新增 `docs/` 和原有未跟踪 `minecraft-protocol/`；已跟踪源码及根 `README.md` 无修改 |
| `git -C minecraft-protocol status --short` | 0 | 无输出，fork 工作树干净 |

校验是临时只读命令，没有新增测试脚本。未执行 Cargo/Gradle 构建、Native 服务启动或浏览器验收；文档变更不代表运行组件已实现。

GraphFlow 当前仅提供 `gf_read`、`gf_edit`、`gf_exec`、`gf_macro`；已使用 `gf_read` 定位源码，未提供 `graphflow_context`、`graphflow_plan` 或 `graphflow_index`，因此未执行图谱刷新。
