# 第三方来源

| 组件 | 固定方式 | 许可证/来源 |
| --- | --- | --- |
| HarfBuzz 14.4.0 | Git submodule `36cb489cb02ce4b92099669ba9f9bea348eff93f` | `vendor/harfbuzz/COPYING`，及子目录中的许可；https://github.com/harfbuzz/harfbuzz |
| ass-core 0.1.2 | Git revision `5c944eaaff75d4fa957d965964e6c039a183b6b8` + Cargo.lock | 上游 workspace 许可；https://github.com/wiedymi/ass-rs |
| ASS 附件 6-bit 编码器 | `crates/ass/src/attachment.rs`，改写自 Aegisub 的 UUEncode 实现 | Copyright (c) 2013 Thomas Goyne；宽松 ISC 风格许可全文保留在源码中；https://github.com/Aegisub/Aegisub |
| ttf-parser、cc、clap、serde 等 Rust 依赖 | Cargo.lock | 各 crate 的许可与源包；可通过 `cargo metadata --locked` 查看 |
| 测试字体 | HarfBuzz 子模块中已有的 test 数据 | 随相应字体及上游目录记录的许可 |
| libass 0.17.5（仅测试） | 官方 release + SHA256 | ISC；https://github.com/libass/libass |
| Noto Sans / Noto Sans SC（仅测试） | `tests/render/fonts.json` 的 commit、URL、SHA256 | OFL 1.1；下载时同时保存两个上游许可证，不链接进 CLI |
| Zig 提供的 libc++ / libc++abi / libunwind / musl 运行库（Linux） | Zig 0.14.1，见 `scripts/linux-toolchain.json` | 构建时复制各自 LICENSE/COPYRIGHT 和 Zig LICENSE，随 Linux 发布包的 `licenses/` 分发 |

本项目不包含用户系统字体的原始文件。CI 只下载固定来源的开源测试字体；商业字体样例只读取用户提供的路径。Rust FFI adapter 是本项目自有小型桥接，不使用旧版 `hb-subset` crate 的内附 HarfBuzz。
