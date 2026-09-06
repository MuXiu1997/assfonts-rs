# 独立 libass 渲染验证

此测试不链接进 CLI。普通 `cargo test` 不需要 libass，也不需要本机商业字体。

真实字幕的后续三组对照方案见 [渲染回归设计](../../docs/render-validation.md)：完整原始字体为基准，Rust 为主要被测对象，assfonts 为辅助对照。下述是现有小型样例测试，尚未完整实现该方案。

`verify.py` 使用 `tests/fixtures/cff-collection.ass`，需要以下用户自备字体：

- `Hiragino Sans GB.ttc`：包含 W6，匹配 face index 2。
- `Expansiva.otf`：CFF 轮廓。

原始字体不复制进仓库。脚本生成只保留其中一个附件的负向样例，独立 C 程序以五套 libass 上下文比较：原字体、嵌入字体、无字体、仅 TTC 子集、仅 OTF 子集。系统字体 provider 必须关闭，避免回退掩盖错误。

## 构建测试用 libass

从 libass 官方 0.17.5 源码构建；需要测试机已有 FreeType、HarfBuzz、FriBidi、pkg-config 和 C 编译器。

```sh
./configure --disable-shared --enable-static \
  --disable-fontconfig --disable-coretext \
  --disable-require-system-font-provider --disable-asm
make -j4
```

官方 release 压缩包 SHA256：`caab4b993dd7be6187c55623b789ed75dddefea6e65938af134637c732fe094a`。

在项目根目录编译验证器，替换下列 `/path/to/libass-0.17.5`：

```sh
mkdir -p .validation
clang -std=c11 -Wall -Wextra -O2 tests/render/verify_libass.c \
  -I/path/to/libass-0.17.5/libass \
  /path/to/libass-0.17.5/libass/.libs/libass.a \
  $(pkg-config --cflags --libs freetype2 harfbuzz fribidi) \
  -liconv -lm -o .validation/verify_libass

python3 tests/render/verify.py \
  --binary target/release/assfonts-rs \
  --oracle .validation/verify_libass \
  --ttc '/System/Library/Fonts/Hiragino Sans GB.ttc' \
  --otf '/Library/Fonts/Expansiva.otf' \
  --output .validation/render-run
```

输出目录必须尚不存在。四个时刻为 500/1500/2500/3500 ms，比较 1280×360 原始 RGB 像素。正向差异必须为 0，且画面必须有文字；无字体画面必须为空；移除 TTC/OTF 的对应帧必须变化。任何断言失败返回非零退出码。

此测试只证明指定样例在指定 libass 版本上的效果，不覆盖所有播放器、标签或字体格式。

## 竖排与 Aegisub 扩展段回归

使用相同字体和验证器，在上述 `verify.py` 命令中增加 `--fixture tests/fixtures/vertical.ass`，并选择新的输出目录。该样例覆盖样式中的 `@`、覆盖标签中的 `\fn@`、横竖排切换和样式重置，以及保留 Aegisub 元数据段。基准加载完整字体，被测组只加载 Rust 字体附件；无字体和单字体负对照保持不变。
