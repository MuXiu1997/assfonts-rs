# 缺字策略与固定默认字体环境

默认 `warn`：保留请求名称对应的候选和缺字 warning。这是固定环境下保留原显示的策略，不是自动补字，也不自动加载默认字体。找不到请求字体、无效输入和子集失败仍报错。
显式 `error` 恢复严格缺字检查。

```sh
# 默认 warn；也可显式添加 --missing-glyphs warn
assfonts-rs -i input.ass -f fonts -o output --json
# 严格模式；--check 同样接受此选项
assfonts-rs -i input.ass -f fonts --check --missing-glyphs error --json
```

`--check` 只规划并返回 warning，不验证最终渲染。
Rust 的 `MissingGlyphPolicy::default()`、`Processor::process(text)` 和内置
`HarfBuzz::subset` 均使用 warn；严格调用使用
`Processor::process_with_policy(text, MissingGlyphPolicy::Error)` 或
`Subsetter::subset_with_policy(face, chars, MissingGlyphPolicy::Error)`。
低层 `FontResolver::resolve` 仍是严格单 face 原语；默认处理走支持策略的 `plan`。
自定义 resolver/subsetter 若没有实现 warn 支持，会明确返回不支持；可显式选择 error，
不会静默降级。直接调用 subset 不返回结构化 warning；需要报告时使用 Processor。

WASM 新引擎及参考宿主 `MemoryEngine` 默认 warn，不自动配置宿主字体。
`af_set_missing_glyph_policy(engine, 0)` / `engine.setMissingGlyphPolicy('error')`
选择严格模式，`1` / `'warn'` 恢复 warning。非法值不改变策略；C ABI 调用会替换
上一条 JSON 响应。严格 TTC/CFF golden 测试显式选 error，哈希保持原值。

## 实现约束

- warning 模式保留与请求名称匹配的全部候选 face，按 catalog 添加顺序嵌入；
  同一源 face 合并字符需求。不预先删掉渲染器可能选择的同名字体候选。
- 所有请求字符都进入 HarfBuzz 的 Unicode/规范化闭包；允许源 cmap 缺字，
  但源字体实际有的所需字符在子集输出中仍必须存在。
- 保留 `.notdef` 轮廓、字体名称及既有布局兼容策略；不替换字幕字符、字体名，
  不把原字体整份嵌入来回避子集错误。
- 找不到请求字体、字体损坏、子集化失败和不支持的字幕语法仍然是错误。
- 只有旧编码 cmap（如 GBK/Big5）或没有 Unicode cmap 的字体仍报错；libass
  可能转码后取到字形，不能将这种情况当作普通缺字降级，否则会改变显示。
  GBK 的原因与独立修复计划见 [后续事项](known-issues/legacy-gbk-cmap.md)。
- 严格模式保持原有子集 flags 与附件排序，避免改变既有输出。

## 宿主的责任

此选项本身不加载或配置默认字体。要主张“固定环境下保真”，必须在完整字体基线
和子集输出两侧使用同一渲染器及配置，并提供相同的默认字体文件、face、版本和
默认 family。libass 需要真正加载字体并设置 `ass_set_fonts` 的默认 family，
仅声明名称或将默认字体作为普通附件并不等价。

验证时关闭系统 font provider，确保完整字体基线的注册顺序与处理 catalog 一致。
CLI 按规范化路径排序；WASM 按 `af_add_font` 顺序。不能比较采用不同顺序的同名字体库。
保留候选只覆盖请求名称对应的字体，不承诺复现任意系统字体库/任意播放器的回退。
若默认 family 与输入库存在同名但不同内容的字体，也必须单独验证选择优先级。

## 报告

schema 1 新增 `missing_glyph_policy` 和 `warnings` 字段；严格模式 warning 数组为空。
每条 `missing_cmap_glyphs` 记录请求、源文件、face index，以及：

- `characters`：该候选缺少非零 cmap 字形的请求字符，不含 default-ignorable 字符。
- `missing_from_all_candidates`：这个请求的所有同名候选都缺少的字符。

这些是静态 cmap 事实，不表示渲染器必然进行了回退，也不保证默认字体覆盖这些字。
Unicode shaping 的规范化也可能影响实际字形选择。像素一致不等于文字全部可读；
渲染验证应同时记录默认字体选择和最终仍找不到 fallback 的日志。
