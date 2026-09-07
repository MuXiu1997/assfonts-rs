# 接口与扩展

## 数据流

`CLI 文件读取 → SubtitleCodec.analyze → FontResolver.resolve → 按源字体 SHA256 + face index 合并 → Subsetter.subset → SubtitleCodec.embed → CLI 原子写入`

核心不包含路径遍历、文件读取、日志输出或 HarfBuzz 类型。`FontFace.source` 只是报告用的标签；字体来自 `Arc<[u8]>`。子集和字幕结果为调用者拥有的内存。将来做 WASM 时，可以复用这些输入输出，另外适配浏览器文件选择和原生后端构建；当前尚无 WASM 实现。

## 插件契约

三个 trait 均为 `Send + Sync`、支持 trait object：

- `SubtitleCodec`：从原始文本得到每种 `FontRequest` 的 Unicode 集合，并完成附件插入。无法分析的语法必须报错。默认 ASS adapter 保留原文，避免解析器序列化改变时间轴或样式。
- `FontResolver`：`resolve` 保留严格单 face 检查；`plan` 按策略规划候选。默认 warn 保留同名候选和注册顺序、记录缺字，显式 error 同分时先载入优先且缺字报错。不会自动加载默认字体。
- `Subsetter`：接收一个 face 与合并后的字符集合，返回独立 SFNT。必须保留渲染所需的名称、布局依赖和覆盖。默认 HarfBuzz adapter 在子集化前后验证覆盖。

`Processor` 借用这些实现，不取得其所有权。更换 resolver 时可自行实现字体缓存、数据库或延迟加载；更换 subsetter 不影响 ASS 状态跟踪与文件输出。

## 增加子集化后端

1. 新建一个 crate，实现 `assfonts_core::Subsetter`。用 `FontFace.data` 和 `index` 读取输入；不要在接口中引入后端私有类型。
2. 在 CLI 中增加可选依赖及同名 Cargo feature。
3. 在 `crates/cli/src/backend.rs` 的 `create` 和 `available` 注册名称。
4. 运行现有管线、TTF/CFF/TTC 和渲染测试，再补后端特有格式的测试。

这是编译期可插拔方案。未启用的后端不进入 CLI 依赖图。示例：`cargo check -p assfonts-cli --no-default-features` 保留分析/检查模式，不构建 HarfBuzz。

核心测试中的 `Codec`、`Resolver`、`Backend` 展示完全独立于原生库的注入与同 face 字符合并。它们不参与字体正确性验证；真实字体测试在 fonts/harfbuzz/cli crate 中。

## FFI 与源码更新

`assfonts-harfbuzz/src/bridge.cc` 用 HarfBuzz 头文件编译四个固定 C ABI 函数。Rust 只处理输入切片、长度与不透明 blob 指针，返回值由 RAII 释放。无需全量 bindgen；`unsafe` 只出现在此 adapter，其他运行时 crate 均 `forbid(unsafe_code)`。

HarfBuzz 通过 Git 子模块固定 commit，`build.rs` 编译其 amalgamation `harfbuzz-subset.cc` 和 bridge。更新时单独提交 gitlink 变更，检查桥接头文件兼容性、执行所有测试和独立渲染验证。禁止在 build.rs 中联网自动拉最新源码。

## 默认策略

- 名称：case-insensitive 匹配 legacy family、full name、PostScript name、typographic/WWS family；名称来源限定为可解码的字体 name 记录。
- 匹配：斜体不匹配惩罚高于字重差，同分采用 libass 的先载入优先规则，再按 collection face 顺序；相同文件字节去重。这里只对齐同分处理，评分及名称匹配尚不等同于 libass 的完整实现。调用方需固定字体添加顺序；CLI 按规范化路径排序。
- 字符：error 要求普通可见字符具有非 `.notdef` cmap 映射；默认 warn 允许源 cmap 缺字、保留 .notdef 轮廓并验证源字体已有的所需字形；Unicode default-ignorable 字符保留在子集输入中，但不强制有独立 cmap glyph。
- 输出：使用子集内容哈希生成 ASCII 附件名；不修改字体内部名称。相同 face 的多个名称/样式需求合并。
- 报告：schema version 为 1；每个字体记录所有请求、源文件哈希、face index、字符集合与子集哈希。
- I/O：默认拒绝覆盖；预检查全部目标、处理全部输入后，再逐个原子发布。单文件原子性不等于整个批次原子性。

## 后续扩展点

优先补真实字幕语料与差分渲染测试，再扩充字体状态变换、已有附件处理及旧编码输入。大字体库的按需读取和内容寻址缓存可作为 `FontResolver` adapter 演进。skera 等后端只有在覆盖与渲染测试通过后，才应成为默认选择。
