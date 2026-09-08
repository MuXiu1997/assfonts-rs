# ASS 解析兼容性

依赖使用 [MuXiu1997/ass-rs fork](https://github.com/MuXiu1997/ass-rs/tree/fix/libass-tag-compat)，
固定完整 commit SHA：`3e7b1c2ab50e078badc215d1a8d9a503a7956abd`。
不跟踪可变分支，也没有创建上游 PR。

## 职责边界

- ass-core 继续负责段结构、样式和事件字段解析，依赖仍固定在上述 fork。
  fork 中既有的颜色边界、空标签恢复和 UTF-8 诊断修复保持不变；
  主项目的字体分析现在使用 `override_tags`，不再将通用标签解析器的名称/括号边界当作 libass 边界。
- assfonts-ass 按 libass 0.17.5 处理 Dialogue 样式和命名重置：
  Dialogue 名称忽略前导星号，仅 Default 不区分大小写；重名样式最后一条生效；
  未知 Dialogue 样式回退 Default，没有显式 Default 时使用 libass 的隐式 Arial/200。
  命名重置精确匹配，找不到时回到该 Dialogue 的原始样式，而非最近一次重置样式。
- 覆盖标签按 libass 0.17.5 的大小写敏感、有序前缀规则识别：`\border` 是 `\bord`，
  `\blu`/`\bklur` 是 `\b`，不能一概丢弃。未匹配任何已知前缀的标签先消耗括号参数再忽略；
  参数里的反斜杠不泄漏为独立字体切换。空标签及反斜杠后的空格/Tab 按 libass 扫描。
  段结构和事件字段诊断仍会报错，覆盖块缺少 `}` 仍拒绝，`\fe` 编码切换仍不支持。
  带星号的 Style 定义仍受 ass-core 继承扩展诊断约束，不宣称已兼容。
- 第一条段标题之前的前言仅从分析视图屏蔽，保留长度和行位置；不将其中的 WrapStyle 等字段当作 Script Info。
  遇到任何段标题即停止前言屏蔽，未知/畸形段标题及段内的其他诊断照常报错。
- 正文的字面 Tab 按 libass 的空格处理，DEL（U+007F）则进入字体字符需求，不擅自删除。
  DEL 可以映射到可见字形；缺字时使用既有 warn/error 策略。NUL 和其他未验证的控制字符继续拒绝。
- `\b` 的整数参数中，0/1 保持常规/粗体语义；其他小于 100 的值（包括负数及 2/4/20）
  回退当前生效样式的 Bold，不沿用前一覆盖值，不影响字体名或斜体；命名重置后的样式同样适用。
  100–900 的显式字重保持支持，大于 900 仍拒绝。覆盖标签整数按十进制前缀解析并钳制至 i32，
  无数字前缀时为 0；无效 Italic/WrapStyle 参数回退当前样式/脚本值，区分空参数与数值 0。
- 括号参数按 libass 的第一个右括号切分，而非平衡嵌套。transform 无有效内部标签时不增加字体需求，
  缺右括号时扫描至覆盖块末尾；支持只含视觉标签的嵌套动画，最多 64 层。
  内部字体/状态切换仍拒绝；第一个右括号后位于动画外的字体切换正常分析。
  不补写括号，也不删除原始未知标签或动画。

这里只分析字体需求；不会规范化或删改原始颜色、空标签、样式或正文。
尤其不能简单删除颜色错误：无包裹参数仍可能表示透明、颜色重置等有效状态。
默认 warn 缺字策略、完整字体回退限制和字体载入顺序均不变。

## TDD

fork 保留可检出的 RED/GREEN 提交，详见
[TDD 记录](https://github.com/MuXiu1997/ass-rs/blob/3e7b1c2ab50e078badc215d1a8d9a503a7956abd/docs/libass-tag-compat-tdd.md)。

| 阶段 | 提交 | 实际回放 |
| --- | --- | --- |
| 空标签 RED | `8a63527` | 4 失败、1 通过 |
| 空标签 GREEN | `c7f8e0a` | 5 通过 |
| 颜色 RED | `e60b205` | 4 失败 |
| 颜色 GREEN | `ba6bf51` | 两组共 9 通过 |

主项目的样式与接入测试同样先复现失败，再实现修复。
WASM 用例与独立 libass 渲染入口见 [验证说明](../tests/wasm/README.md)。
本轮 229 份真实字幕及渲染结果见 [2026-09-08 验证记录](validation/2026-09-08-parser-compatibility.md)。
随后针对控制字符/段前言的 3 份修复见 [后续验证记录](validation/2026-09-08-controls-structure.md)。
低字重回退的 14 份目标见 [字重验证记录](validation/2026-09-08-weight-fallback.md)：
新增 12 份成功，另 2 份仍被 transform 语法阻挡；累计 158/229 成功。
这两份随后通过无内部标签/缺右括号的 transform 恢复规则解决，并带来 8 份同类成功，
累计 168/229；详见 [transform 恢复验证](validation/2026-09-08-transform-recovery.md)。
随后 42 份覆盖标签与 17 份动画问题全部解决，累计 227/229，剩余两份缺字体；
详见 [覆盖标签与嵌套动画验证](validation/2026-09-09-override-animation.md)。
