# 验证记录

记录实际执行范围；规划中的运行验收条件见各主题文档，不代表已经通过。

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
