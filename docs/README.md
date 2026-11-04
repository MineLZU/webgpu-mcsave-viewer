# 文档索引

| 问题 | 文档 |
|---|---|
| 展示器目标、边界和实施顺序是什么？ | [1.21.11 架构方案 v2.1](wgpu_mc_1_21_11_web_viewer_architecture-v2.md) |
| 工作区已有哪些组件，计划组件放在哪里？ | [工程分层](architecture.md) |
| Kotlin 协议库 fork 复用什么、改了什么？ | [fork 集成边界](viewer-fork-integration.md) |
| 服务端热更新如何可靠到达浏览器？ | [捕获与同步](viewer-live-sync.md) |
| 展示协议每条消息的字节怎么排？ | [展示协议 v1](viewer-protocol.md) |
| 状态变化如何转成正确的网格？ | [状态与烘焙调度](viewer-state-scheduling.md) |
| 现有渲染器如何迁移到浏览器？ | [WebGPU 与资源适配](viewer-webgpu.md) |
| 未加载区块如何与热状态共存？ | [冷区块读取](viewer-cold-chunks.md) |
| 哪些检查实际执行过？ | [验证记录](build.md) |

方案描述目标行为；源码现状与已知偏差分别标明。运行组件尚未接入，文档中的拟建目录和接口名称不代表已有实现。
