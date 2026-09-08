# 两类 transform 恢复验证（2026-09-08）

本轮仅修改主项目 `assfonts-ass`，基于 `4bcc769b2861edcf9e01c4a44dc998b76d033042`；
transform 修复尚未提交。fork SHA、Cargo.lock 和字体策略不变。
机器可读结果见 [JSON](2026-09-08-transform-recovery.json)。

## 两个目标与语义

| 样本 | 旧错误 | 已验证的处理 |
| --- | --- | --- |
| s007-0027 | dialogue 34: transform without tags | `\t(100,6590)` 不含内部标签，不增加字体需求 |
| s007-0030 | dialogue 514: malformed transform | `\t(37,750,\c&HFAFAFC&` 缺右括号，检查内部标签直到覆盖块末尾 |

本地固定的 libass 0.17.5 `libass/ass_parse.c` 在参数切分阶段允许缺失右括号（约 340–349 行），
在 transform 分支中跳过没有反斜杠参数的内部解析（约 710–715 行）。
分析器采用相同的字体需求处理方式，不修改原始字幕，不补写括号，也不删除 transform。
无内部标签的 transform 仍可能影响渲染器的碰撞状态，因此不能声称可从原文安全删除。

仍要求 transform 参数以 `(` 开始，并检查所有解析出的内部标签。
字体/状态切换、未知标签及嵌套 transform 的拒绝规则保持不变；本轮不宣称完整 transform 兼容。

## TDD 与 WASM

- 新测试在旧实现上为 2 失败，原视觉 transform 测试为 1 通过；修复后通过。
- 覆盖空参数、仅时间参数、无标签且缺右括号、颜色/透明度/缩放/模糊标签缺右括号，以及下一覆盖块的字体切换。
- 有/无右括号两种形式均验证 `\fn`、`\b`、`\i`、`\r`、`\p`、未知标签、嵌套 `\t` 仍被拒绝。
- workspace 共 63 项测试通过；fmt、Clippy all-targets `-D warnings`、Deno 类型检查通过。
- 实际 Worker 新增两个持久变体，子集哈希保持基准值，ASS 原文保留。
- 独立 libass 渲染：12 组、600 个采样时刻 RGBA 零差异，缺附件负对照通过。

## 全量真实样本与影响范围

同一新 WASM 构建实际重新处理全部 229 份：168 成功、61 失败。
两个指定目标均成功；同样的两条恢复规则还使 8 份同类样本成功，无需增加其他规则。
新增成功清单：`s005-0007`、`s007-0027`、`s007-0030`、`s022-0030`、`s024-0001` 至 `s024-0006`。

全部 229 份输入 SHA-256 已验证，168 份成功输出均保留原始 ASS。
此前 158 份成功字幕及 JSON 报告逐字节相同；剩余 61 份错误文本不变，无新的回归失败。

新增成功的 10 份全部完成原字体/子集对照：libass 0.17.5，1920×1080，provider NONE，
两侧固定 Noto Sans SC 默认字体，共 41,963 个采样时刻 RGBA 零差异。
两侧渲染错误及 fallback failure 均为零，各样本统计见 JSON。

旧 158 份本轮仅验证输出/报告逐字节一致，未重跑其全部真实渲染。
真实处理仍使用既有 warn 缺字策略，不宣称跨平台或完整渲染器兼容。

## 可追溯性

- WASM SHA-256：`fa876d36db891c2fab5ecdf1d873e3b3974abe024a6e4520258d484e859d037b`。
- 构建清单全部源文件、产物和 Cargo.lock 哈希已与当前文件核对通过。
- 本地批量处理、校验脚本和真实渲染结果：`/Users/muxiu1997/Documents/Codex/2026-09-07/assfonts-rs-wasm-ass-processor-lib/work/transform.YUMbCn`。
- Worker/合成渲染结果：`/Users/muxiu1997/Projects/assfonts-rs/.validation/mise-wasm-hehctkcn/render`。
- 临时验证脚本使用独立 PEP 723 环境，未增加项目依赖。
