# 低字重参数回退验证（2026-09-08）

本轮仅修改主项目 `assfonts-ass`，基于 `a2ea7b172f3f0dd0f2817959b1e7b1e5e0a47a44`；
字重修复尚未提交。fork SHA、Cargo.lock 和字体策略不变。
机器可读结果见 [JSON](2026-09-08-weight-fallback.json)。

## 语义与边界

依据本地固定的 libass 0.17.5 `libass/ass_parse.c`（825–830 行），
`\b` 的 0/1 分别保持常规/粗体；其他小于 100 的整数回退当前生效样式的 Bold。
不能统一当作粗体，不能沿用前一个覆盖值，也不能无条件回退全局 Default。
命名 `\r` 后使用该样式；未知命名重置仍使用该 Dialogue 的原始样式。
仅重置字重，字体名和斜体状态不变。100–900 保持已有支持；大于 900、其他数字语法、
未知标签和 transform 规则没有扩展。原始字幕不改写。

## TDD 与持久回归

- 新测试先在旧实现失败：`bold weight -100`；实现后两个新测试均通过。
- 参数覆盖 -100、-1、2、4、20、99，含粗体/非粗体样式、命名/未知重置、字体与斜体覆盖。
- 另检查 0、1、100、400、700、900 保持原行为，901 继续拒绝。
- workspace 共 61 项测试通过；fmt、Clippy all-targets `-D warnings`、Deno 类型检查通过。
- 新 WASM 变体分别在粗体 Default、非粗体 CJK、命名重置后使用 20/2/4。
  子集哈希与既有基准一致，输出 ASS 保留原文。
- 实际 Worker 与独立 libass 渲染：10 组、500 个采样时刻 RGBA 零差异，缺附件负对照通过。

## 229 份真实样本

同一新 WASM 构建实际重新处理全部 229 份：158 成功、71 失败，较上一轮净增 12 份成功。
全部输入 SHA-256 已验证；158 份成功输出均保留原始 ASS。
此前 146 份成功的字幕输出及 JSON 报告逐字节一致，其他 69 份失败的错误文本完全不变。

目标 `s007-0023` 至 `s007-0036` 共 14 份均不再被低字重参数阻挡，其中 12 份成功；
另外两份暴露下一项错误，未擅自修复：

| 样本 | 当前剩余错误 |
| --- | --- |
| s007-0027 | dialogue 34: transform without tags |
| s007-0030 | dialogue 514: malformed transform |

不能将这两份计作成功，也不能将本轮称为“14 份全部处理成功”。

## 真实渲染对照

新增成功的 12 份使用 libass 0.17.5，1920×1080、provider NONE，
原字体和 WASM 子集两侧均固定 Noto Sans SC 默认字体。
共 53,274 个采样时刻，RGBA 零差异；两侧渲染错误和 fallback failure 均为零。
各样本采样数和渲染统计保存在 JSON。

旧 146 份本轮仅验证输出/报告逐字节一致，没有重跑其完整真实字幕渲染。
真实处理仍使用原有 warn 缺字策略，不宣称跨平台验证或 libass 全语法兼容。

## 可追溯性

- WASM SHA-256：`f57c3ce3c8bf38bc35d9aa6263329d53627e52a8dcb8362bb9916f986176e46d`。
- 构建清单中的全部源文件、产物及 Cargo.lock 哈希均与当前文件核对通过。
- 本地批量验证与逐样本结果：`/Users/muxiu1997/Documents/Codex/2026-09-07/assfonts-rs-wasm-ass-processor-lib/work/weight.BPYIGI`。
- Worker/合成渲染结果：`/Users/muxiu1997/Projects/assfonts-rs/.validation/mise-wasm-_m1_5liy/render`。
- 临时验证脚本按独立 PEP 723 环境执行，未增加项目依赖。
