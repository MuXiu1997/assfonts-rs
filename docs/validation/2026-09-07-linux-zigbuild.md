# Linux cargo-zigbuild 发布入口验证

记录日期：2026-09-07。机器可读汇总见 [linux-zigbuild.json](linux-zigbuild.json)。

## 实现

- Linux musl 发布统一执行 `python3 scripts/build_linux.py`；固定 Rust 1.92.0、cargo-zigbuild 0.23.4、Zig 0.14.1 和 x86_64 目标。HarfBuzz 使用 Zig 的目标 libc++。
- CI 对该入口生成的同一个二进制执行 SHA-256、ELF 静态链接、实际运行和字体渲染检查，随后直接打包；不在验证后重新构建。macOS、Windows 原生构建 job 保留。
- CI 字体采用固定上游 commit 和 SHA-256 的 Noto Sans TrueType 与 Noto Sans SC CFF。两个完整 TrueType face 打包为 TTC，验证非零 face 选择。libass 0.17.5 源码也校验 SHA-256。
- 独立渲染器改为比较预乘 RGBA，系统字体 provider 为 NONE，保留无字体和移除单字体负对照。四秒样例每 100ms 和事件边界前后 1ms 共 50 个时间点。

## 验证中发现并修复的问题

新增样例的 `a` + U+0301 组合重音在原有 macOS 程序上已有差异：500ms 时 171 个像素不同。改用预组合的 `á` 则没有差异，证明问题不由 Zig 引入。

经用户授权，HarfBuzz adapter 增加 Unicode 规范等价字符闭包：从输入字符的规范分解出发，保留源字体 cmap 中需要的规范组合及分解字符。未使用兼容性规范化，未修改原始 ASS。新增双向字形覆盖单元测试，并保留原分解形式的渲染样例。修复后该样例零像素差。

## 实际执行结果

Linux 二进制由上述正式入口在 Apple Silicon 上构建，SHA-256：

`7e9a7773ae4a36388bf5ecbcc759380eaedc0dfdb42ef7f12667e024fae4638f`

- 在 x86_64 Linux 临时 Alpine 容器运行：ELF 无 `INTERP`/`NEEDED`，后端为 HarfBuzz；测试前后哈希一致。
- Linux 开源字体样例：50 个采样点，47 个非空，RGBA 零差异；无字体/移除单字体负对照通过，基准及被测日志无字体错误。
- 同一 Linux 二进制重新处理既有 49 个真实字幕：43 个成功，6 个既有输入错误保持不变。成功输出在本地 macOS 的独立 libass 0.17.5 中与完整原始字体比较：143445 个采样点、102945 个非空，ASS_Image 链全部精确一致，所有 ASS 原文按字节保留。
- 真实批次日志仅保留 `s03-t004`、`s03-t017` 两个既有空事件的 glyph 0x0 警告，基准和被测均出现，无新增错误。
- macOS 开源字体、既有商业 TTC/CFF、竖排三个样例均通过升级后的 RGBA 检查。原生链接仍只有系统 libc++、libiconv、libSystem。
- 20 个 Rust 测试、fmt、clippy `-D warnings`、禁用原生后端检查通过。Actionlint 1.7.7 通过（未启用 ShellCheck），shell 步骤另经 `bash -n` 检查。
- 发布 tar 包中的二进制 SHA-256 与已验证文件一致，保留可执行权限，附带构建清单及运行库许可。

Linux 渲染器使用 libass 0.17.5；FreeType 的 pkg-config 版本为 26.6.20（软件包 FreeType 2.14.3）、HarfBuzz 12.2.0、FriBidi 1.0.16。这些仅属于独立渲染器；CLI 内嵌的 HarfBuzz 仍为固定的 14.4.0。

## 证据边界

当前 Git 仓库没有远程地址，未触发 GitHub Actions。以上是本机交叉编译和真实 Linux 执行结果，不声称 Ubuntu 24.04 的远程 CI 已通过，也未重新运行 Windows 构建。

公共 CI 使用真实开源字体和人工样例，不依赖商业字体、NAS 或私有字幕。43 个真实样本的完整回归是本次额外本地验证，不能将 50 个 CI 采样点描述为每次都会重跑全部 143445 个采样点。

详细批次记录保存在工作流项目的 `outputs/assfonts-linux-build/20260907/`；原始字体及字幕留在忽略的验证目录，不提交到仓库。
