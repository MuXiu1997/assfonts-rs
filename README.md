# assfonts-rs

模块化的原生 ASS 字体处理 CLI：分析字幕、匹配字体、通过静态链接的 HarfBuzz 子集化，再将字体嵌入 ASS。

当前为可运行的 **0.1.0 初始实现**。已验证 macOS / Apple Silicon 和 Linux x86_64 musl 单文件运行，使用者无需安装 HarfBuzz、FreeType、libass、Python 或 Rust。字体文件是用户提供的输入。完整兼容范围见下文；尚未覆盖原 assfonts 的全部功能。

另提供 **实验性 Emscripten 单模块 WASM**，复用相同处理管线；服务器 Deno 2.5.6
已实际执行验证。构建、测试与已知内存限制见 [WASM 验证说明](tests/wasm/README.md)。
它不会自动替换现有 Deno 字幕库或原生 CLI。

## 构建

开发者可从 [mise 统一开发入口](docs/development.md) 开始：安装固定工具后使用
`mise run check`、`mise run test:native`、`mise run build:linux` 或
`mise run build:wasm`。下面的独立 Cargo/脚本命令仍可使用。

需要 Rust 1.92+、Git 和 C++17 编译器。macOS 使用 Xcode Command Line Tools。无需 bindgen、libclang、pkg-config 或系统 HarfBuzz。

```sh
git submodule update --init --recursive
cargo build --release --locked
./target/release/assfonts-rs --help
```

新克隆时使用 `git clone --recurse-submodules <repository-url>`。`vendor/harfbuzz` 固定于 **14.4.0 / 36cb489cb02ce4b92099669ba9f9bea348eff93f**；Cargo 构建不下载或更新其源码。

可执行文件在 `target/release/assfonts-rs`（Windows 为 `.exe`）。macOS 保留系统 `libc++`、`libiconv`、`libSystem` 依赖。Linux 完全静态构建和 Windows CRT 配置见 [构建与发布](docs/building.md)。

## 使用

```sh
# 可以直接用仓库内的测试字体试运行
./target/release/assfonts-rs \
  -i examples/basic.ass \
  -f vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf \
  -o .validation/example \
  --report .validation/example/report.json

# 批量处理：输入目录和字体目录递归扫描，可重复指定
./target/release/assfonts-rs \
  -i /path/to/subtitles \
  -f /path/to/fonts -f /another/font.otf \
  -o /path/to/output -v2

# 检查语法、字体匹配和字符覆盖；不子集化、不创建任何输出
./target/release/assfonts-rs \
  -i /path/to/subtitle.ass -f /path/to/fonts --check --json
```

输出为 `<输入文件名>.assfonts.ass`；省略 `-o` 时写入输入文件所在目录。保持原字幕正文、样式、行尾和 BOM，仅新增 `[Fonts]`。目录扫描自动跳过已有 `*.assfonts.ass`，显式输入已嵌入字体的 ASS 会报错。

- `--report <path>`：写出整个批次的 JSON 报告，包括来源、face index、字符集合和 SHA256。
- `--json`：将报告输出至 stdout；进度和错误始终在 stderr。
- `--overwrite`：显式替换已有结果/报告；输入字幕及字体不允许被输出覆盖。
- `--backend harfbuzz` / `--list-backends`：选择或列出编译进来的子集化后端。
- `--missing-glyphs error|warn`：默认 `warn` 保留候选及缺字 warning，保真要求宿主维持同一默认字体/渲染环境；显式 `error` 恢复严格缺字报错。详见 [缺字策略](docs/missing-glyph-policy.md)。
- 退出码：成功 `0`，处理或 I/O 错误 `1`，命令行参数错误 `2`。

默认不覆盖已有文件。每个结果通过同目录临时文件原子发布；全部字幕成功处理后才开始写出。发布阶段若遇到磁盘或权限错误，之前发布的结果仍会保留，错误信息会说明进度；整个批次不是跨文件事务。无法解析的字体仍会报错；缺字默认记录 warning，严格 `error` 模式报错；同分候选按明确的字体载入顺序选择。

## 模块与插件

| crate | 职责 |
| --- | --- |
| `assfonts-core` | 纯内存数据模型、处理管线、`SubtitleCodec` / `FontResolver` / `Subsetter` 接口 |
| `assfonts-ass` | 基于 `ass-core` 解析语法，跟踪字体状态，编码和插入 ASS 附件 |
| `assfonts-fonts` | 基于 `ttf-parser` 建立字体目录索引、匹配名称/字重/斜体、验证覆盖 |
| `assfonts-harfbuzz` | C++17 静态构建与小型 C ABI，封装 HarfBuzz 子集化 |
| `assfonts-cli` | 文件系统、参数、批处理、输出策略、后端注册 |
| `assfonts-wasm` | 实验性纯内存 C ABI，供 Emscripten 单模块宿主调用 |

插件以 **trait 注入 + Cargo feature** 的形式工作，随程序静态编译。嵌入应用可直接提供自己的实现；CLI 增加后端时只需添加 adapter crate、可选依赖和注册项。没有运行时 `.so` / `.dll` 插件加载，也没有不稳定的 Rust 动态 ABI。详见 [接口与扩展](docs/architecture.md)。

## 当前兼容范围

- UTF-8 ASS v4+，Unicode/中英文字体名，TTF、OTF、TTC、OTC。
- Aegisub 的 Project Garbage、Project、Extradata 段仅在字体分析时跳过，输出原样保留；其他解析诊断仍会报错。
- 使用到的 Dialogue 字符，`\fn`、`\b`、`\i`、`\r`、命名样式重置。
- `\N`、`\n`、`\h`、`\q`，绘图模式 `\p`；绘图坐标不作为字体字符，但非空绘图段仍收集当前字体、字重和斜体依赖。
- 常用位置、颜色、缩放、描边、淡入淡出、卡拉 OK 标签，以及不改变字体选择的 `\t`。
- 样式和 `\fn` 的 `@` 竖排字体：查询同一物理字体，合并横排/竖排字符需求，输出保留原始 `@` 布局标记。
- 字体按名称、字重和斜体评分；允许播放器合成粗体/斜体所需的最近 face。同分时采用 libass 的先载入优先规则，再按 TTC face 顺序；CLI 使用规范化路径排序，内存 API 使用调用方添加顺序。报告记录实际来源和 face index；不假定同名字体轮廓相同。
- 同一源 face 的需求合并后只子集化一次。保留字体名称（包括本地化/legacy）、所有布局 feature、默认布局闭包和 hinting；重新验证输出字符覆盖。
- 子集保留源字体中 Unicode 规范组合/分解所需的字符，例如 `a` + 组合重音对应的 `á`，避免 shaping 规范化后改变显示；不修改字幕文字，不使用兼容性规范化。
- 默认额外保留源字体已有的 U+0020–U+00FF、U+FF01–U+FF5E 和 U+3000（常见拉丁字符、数字、空格、符号及全角 ASCII）；这些可选字符缺失不会报错，字幕实际字符缺失在 `error` 模式报错，在默认 `warn` 模式记录 warning。
- 旧式 AAT `mort` 支持非上下文替换：收集替换字形闭包并重编号表，避免丢表后切换至不同的 GSUB 行为；不回退嵌入完整字体。不支持的 `mort` 状态机或损坏表明确报错。详见 [子集兼容策略](docs/subsetting-compatibility.md)。
- 可选 `BASE` 表若被当前固定 HarfBuzz 的渲染校验器判定无效，则在子集前移除该无效表；合法表照常子集化，不扩大到其他布局表。该策略解决梦源字体错误的 BASE 版本/偏移，不使用完整字体回退。

**明确拒绝**：已有 `[Fonts]`、非 UTF-8、SSA/v4++、未知样式、无法识别的覆盖标签、`\fe`，以及改变字体状态的 `\t`。不能可靠处理的输入返回错误，不输出猜测结果。

旧编码目前支持 [Microsoft PRC/CP936 format-2 cmap](docs/legacy-cmap.md)：在内存中补充等价 Unicode 映射后正常子集化，原生/WASM 共用固定映射，不依赖系统 iconv，不修改原字体。

尚未实现：自动系统字体回退、其他旧编码转换、字体内部重命名、已有附件合并、跨字幕缓存、跨集共享字体和动态插件。字体文件目前全部加载到内存；批处理结果也先保留在内存，因此超大字体库/批次需要分批调用。可变字体、CFF2、彩色字体没有专项渲染验证；不能将格式可解析等同于全量兼容。

## 验证

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
# 核心与检查模式可以完全移除原生后端
cargo check -p assfonts-cli --no-default-features --locked
```

自动测试使用固定 HarfBuzz 子模块内的测试字体，不依赖本机字体。Linux CI 使用 cargo-zigbuild 的实际发布产物，检查静态链接，并用固定来源的 Noto 开源字体进行独立 libass 渲染及负对照测试；商业字体样例保留为可选本地检查，见 [渲染测试说明](tests/render/README.md)。2026-09-06 本地测试和链接检查结果见 [验证记录](docs/validation/2026-09-06.md)。

HarfBuzz 源码与许可证位于子模块；其他依赖固定在 `Cargo.lock`。见 [第三方来源](THIRD_PARTY.md)。
