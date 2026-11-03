# 现有渲染器如何迁移到浏览器

## 复用基础与必要适配

| 能力 | 当前依据 | 实现要求 |
|---|---|---|
| GPU API | `wgpu-mc/Cargo.toml:13` 使用 `wgpu 30.0.1` | 实测 `wasm32-unknown-unknown`；异步创建 adapter/device/surface，处理 resize、错误与设备丢失 |
| 绘制参数 | `wgpu-mc/src/render/graph.rs:1541` 检查 `IMMEDIATES` | 浏览器图和 WGSL 改用 uniform/storage；不能因缺少该 feature 而静默跳过地形管线 |
| 顶点格式 | `wgpu-mc/src/render/pipeline.rs:122` 为 16 字节 | 保留精度和字段契约；删除原稿未经验证的 8 字节替代布局 |
| 缓冲池 | `wgpu-mc/src/mc/chunk.rs:266` 使用 `RangeAllocator` | 复用已有 arena，按设备限制与内存预算扩展；不固定 128MB 或另换 allocator |
| 区间回收 | `wgpu-mc/src/mc/chunk.rs:293` 延迟归还旧区间 | 浏览器侧依据提交完成关系确认安全；不能只切换偏移就立即复用仍被 GPU 读取的空间 |
| 绘制基础 | 现有地形分层与逐段路径 | 首期普通 indexed draw，CPU 视锥剔除；间接绘制及相关 feature 仅作为后续能力分支 |
| 半透明 | `wgpu-mc/src/mc/chunk.rs:2388` 已有 quad 排序 | 验证并补齐摄像机移动触发的索引重排；复用 solid/cutout/translucent 分层 |

## 资源和游戏输入替代

| 输入 | 浏览器方案 |
|---|---|
| 同步资源接口 | 先异步下载带清单和哈希的资源，再以内存 provider 实现 `wgpu-mc/src/mc/resource.rs:77`；资源未就绪时不开始烘焙 |
| 方块与状态 | 服务端导出实际注册表和可在服务端取得的状态属性；资源工具处理 1.21.11 模型，生成可核验的映射 |
| 遮挡与流体 | 补齐 `FaceFlags`、流体种类/高度等输入；服务端属性与客户端模型属性分工，不能要求 dedicated server 运行客户端渲染器 |
| 光照与着色 | 使用捕获的光照与 biome 数据；原版着色规则由匹配版本的资源/算法提供，不能用常量颜色冒充全部 biome |
| 模型变体 | 保留 variant/multipart，按目标版本验证确定性选择与位置偏移；原先由 JVM 供给的结果须有新来源 |
| 图集与动画 | 创建浏览器自己的图集并解析动画元数据；`wgpu-mc/src/render/pipeline.rs:43` 的游戏图集标志不能继续依赖不存在的客户端纹理 |
| 缺失内容 | 显示可识别占位并保留状态身份；特殊模型、方块实体渲染或模组客户端代码不假定能由 JSON 重建 |

## 基础运行与验收

- P0 先验证最小地形、实际 shader/图配置和异步资源链路；现有 `resources/` 不能视作完整的 1.21.11 资产包。
- 首期单线程 WASM，审计线程、锁和阻塞调用；后续启用 `wasm-bindgen-rayon` 时，由服务器发送 COOP/COEP 并处理跨域资源策略。
- 实测 Chromium WebGPU：部分方块精度、边界 AO、玻璃/水排序、纹理动画、设备限制、频繁替换网格及设备恢复；现有 shader 测试不能替代浏览器验收。

## 已知偏差

桌面路径与外部 JVM 提供了部分资源、状态和可见性输入；`wgpu-mc/src/mc/mod.rs:824` 的模型烘焙仍要求先注册图集。当前没有浏览器运行入口；Native 编译成功、线程池和性能提升均须独立验证，不承诺零拷贝、固定帧率或固定节省比例。
