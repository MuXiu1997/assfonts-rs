# 默认严格语法模式验证（2026-09-09）

基线为 `006b6cf70dd0ec25fdd00ee7bf0a9ce3da2a5f3a` 后的工作区改动。
默认 `strict`，显式 `compatible` 保留已有兼容接受规则；缺字策略仍独立默认 `warn`。
严格模式保留嵌套视觉动画，拒绝未知/疑似拼错标签及既定非标准整数形式。
完整边界见 [解析模式](../parser-compatibility.md)。

## 验证结果

- `mise exec -- cargo test --workspace --locked --quiet`：76 项测试通过。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、
  `cargo check -p assfonts-cli --no-default-features --locked`、`git diff --check`：通过。
- Deno 类型检查及实际 Emscripten Worker：通过；严格拒绝、显式兼容、模式恢复、缺字策略独立、
  report.parse_mode、正文保留、原有子集哈希均验证。
- 独立 libass 渲染：16 组、800 帧采样，嵌入前后 RGBA 差异像素总数 0；
  baseline/embedded errors 均为 0，所有组（含负对照）通过。
- 嵌套视觉动画和第一个右括号后字体切换的渲染用例仍在 strict 下运行；
  未知/拼错标签的兼容正例明确选择 compatible。
- Rust 测试覆盖 64 层内部嵌套（不含事件级外层 t）的允许边界、超过边界的拒绝、
  动画内字体/状态变化拒绝，以及中文文本中嵌套错误的 UTF-8 字节偏移。

实际产物与渲染记录：`.validation/mise-wasm-5opwtyxw/render/`。

| 产物 | SHA-256 |
| --- | --- |
| engine.js | 511a85c31d05e1c738bac07558a445d75598b73f0f0de209636dd1aee8141d49 |
| assfonts-wasm.wasm | 4a753cb9802fc71fd8f176c21423aae37489933cec14b66ca3f99f8b706f708e |

WASM 使用既有固定 Emsdk 构建：默认 `.validation/emsdk` 不存在，实际通过
`scripts/build_wasm.py --emsdk <既有 SDK 路径>` 指定先前使用的 SDK；未安装或升级工具。
随后执行 `mise run test:wasm`。构建清单保存在 `target/wasm/build-manifest.json`。

## 原始《古见》字幕只读复核

对既有完整回归目录中的 `cases/s010-0007/prepared.ass` 和 `cases/s010-0024/prepared.ass`
执行本次 debug CLI 的 `--check -v0`，未指定 parse-mode，使用仓库 OpenSans 字体作为输入。
两者均在字体解析阶段之前因严格语法退出（退出码 1），不写出文件：

| 样本 | 实际错误位置 | 标签 |
| --- | --- | --- |
| 07 集 | dialogue 976，Text 字节偏移 1 | `\blu0.8` |
| 24 集 | dialogue 308，Text 字节偏移 30 | `\bklur0.5` |

错误均提示 unknown tag / possible misspelling of blur，不自动修正。
此检查只证明语法拒绝；未据此声称源字幕字体已完整匹配或进行两集的全片重渲染。
历史 227/229 成功统计属于兼容接受规则，本轮没有重新执行全部 229 份真实字幕渲染。
