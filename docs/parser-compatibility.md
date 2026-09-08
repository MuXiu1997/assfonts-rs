# ASS 解析兼容性

依赖使用 [MuXiu1997/ass-rs fork](https://github.com/MuXiu1997/ass-rs/tree/fix/libass-tag-compat)，
固定完整 commit SHA：`3e7b1c2ab50e078badc215d1a8d9a503a7956abd`。
不跟踪可变分支，也没有创建上游 PR。

## 职责边界

- ass-core 修复颜色/透明度标签名边界，以及空标签恢复时吞掉后续反斜杠的问题。
  畸形多字节字符的诊断按 UTF-8 边界截取，避免 panic；扩展注册表仍优先处理精确匹配。
- assfonts-ass 按 libass 0.17.5 处理 Dialogue 样式和命名重置：
  Dialogue 名称忽略前导星号，仅 Default 不区分大小写；重名样式最后一条生效；
  未知 Dialogue 样式回退 Default，没有显式 Default 时使用 libass 的隐式 Arial/200。
  命名重置精确匹配，找不到时回到该 Dialogue 的原始样式，而非最近一次重置样式。
- 调用方仅接受 span 为单个反斜杠的 EmptyOverride 诊断。
  其他畸形字符、未知标签及会改变字体选择的 transform 继续拒绝。
  带星号的 Style 定义仍受 ass-core 继承扩展诊断约束，不宣称已兼容。

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
