---
name: rust-check
description: 改完 Rust 代码后运行本仓库的本地检查（fmt、clippy、test），并把结果记进 docs/build.md。在交付 Rust 改动前、或用户要求验证时使用。
---

本仓库没有 CI，这些检查是唯一关卡。在仓库根按顺序运行，前一步失败就先修再继续：

1. `cargo fmt --all -- --check`。有差异就运行 `cargo fmt --all` 再复查。
2. `cargo clippy --workspace --all-targets -- -D warnings`。修掉每条警告；不要用 `#[allow]` 压掉，除非用户同意并在对应文档的「已知偏差」写明原因。
3. `cargo test --workspace -- --nocapture`。留意 `modtree` 的跳过信息：没设 `WGPU_MC_MOD_DIR` 且找不到 Neolectrum checkout 时，依赖 mod 文件的测试会跳过，报告时要写明「跳过」而不是「通过」。
4. 只有改动涉及网格生成或性能时，才跑 `cargo run -p wgpu-mc --release --example mesher_bench -- --sections 64 --repeats 5`（需要 GPU）。

每条命令记下退出码、耗时和测试数量，在 `docs/build.md` 加一节（标题带日期），格式沿用该文件已有的表格。不要把验证结果写到其他文档。

向用户报告时如实列出失败和跳过项，附上相关输出。
