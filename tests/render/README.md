# 独立 libass 渲染验证

此测试不链接进 CLI。普通 `cargo test` 不需要 libass，也不需要本机商业字体。

真实字幕的三组对照方案及已有结果见 [渲染回归设计](../../docs/render-validation.md)。这里的 CI 样例使用真实开源字体和人工编写的四秒字幕，不等同于已有 43 个真实字幕的完整批次。

## CI 开源字体回归

Linux CI 对 `scripts/build_linux.py` 生成的同一个二进制执行以下步骤。需要 Python、C 编译器、make、curl、pkg-config，以及 FreeType、HarfBuzz、FriBidi 开发包。

```sh
python3 -m pip install -r tests/render/requirements.txt
python3 tests/render/prepare_fonts.py --output .validation/ci-fonts
sh tests/render/build_oracle.sh
python3 tests/render/verify.py \
  --binary target/zigbuild/x86_64-unknown-linux-musl/release/assfonts-rs \
  --oracle .validation/oracle/verify_libass \
  --fixture tests/fixtures/ci-fonts.ass --ttc-face-index 1 \
  --ttc .validation/ci-fonts/NotoSans.ttc \
  --otf .validation/ci-fonts/NotoSansSC-Regular.otf \
  --output .validation/ci-render
```

`fonts.json` 固定上游 commit、下载 URL 和 SHA-256，同时取得 OFL 许可证。FontTools 4.59.2 将完整的 Noto Sans Regular/Bold TrueType 字体打包为 TTC，Bold 位于 face 1；不做子集化。Noto Sans SC 使用上游完整 CFF OTF 文件。字体数据只下载到忽略目录，不依赖系统字体或私有 NAS。

样例覆盖 TTC 非零 face、CFF 中文、连字、组合重音、合成斜体、黑字和半透明阴影、移动/淡入淡出/变换、字体切换、竖排、样式重置及 Aegisub 扩展段。比较前按字节核对字幕正文和元数据未改变。完整字体仅注入基准上下文；被测组只读取 CLI 生成的字体附件。系统字体 provider 为 NONE。

每 100ms 及整秒事件边界前后 1ms 比较 1280×360 **预乘 RGBA**，要求零差异；四个中点必须非空。无字体组必须为空，移除 TTC/CFF 必须在指定对应帧检测到差异。基准/被测日志中的字体错误或缺字会导致失败。保存原始 RGBA 代表帧（PAM 容器）、渲染报告、处理报告和字体/字幕/二进制哈希；验证期间二进制不得变化。

## 可选商业字体样例

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

输出目录必须尚不存在。上述相同 RGBA 和时间采样规则也用于这两套四秒商业字体样例。500/1500/2500/3500 ms 保存代表帧。正向差异必须为 0，四个中点必须有文字；无字体画面必须为空；移除 TTC/OTF 的对应帧必须变化。任何断言失败返回非零退出码。

此测试只证明指定样例在指定 libass 版本上的效果，不覆盖所有播放器、标签或字体格式。

## 竖排与 Aegisub 扩展段回归

使用相同字体和验证器，在上述 `verify.py` 命令中增加 `--fixture tests/fixtures/vertical.ass`，并选择新的输出目录。该样例覆盖样式中的 `@`、覆盖标签中的 `\fn@`、横竖排切换和样式重置，以及保留 Aegisub 元数据段。基准加载完整字体，被测组只加载 Rust 字体附件；无字体和单字体负对照保持不变。
