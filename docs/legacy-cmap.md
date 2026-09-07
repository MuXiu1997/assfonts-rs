# 旧式 CP936 字体的内存映射

默认 warn 和显式 error 都支持没有 Unicode cmap、且首选 Microsoft cmap 为
PRC（platform 3 / encoding 3）、format 2 的字体。不需要额外 CLI/WASM 开关。
Big5、其他旧编码/格式、缺失 cmap、损坏的 format-2 表仍明确拒绝。

## 处理路径

1. 使用固定的 Microsoft CP936 可逆映射，把 Unicode 字符转成单字节/双字节编码。
   无近似替换、问号替换或 GB18030 私用区重分配，不依赖系统 iconv。
2. 按 format-2 规则查原 glyph ID：区分单字节和双字节，保留零字形缺失语义，
   对 idDelta 做 16 位回绕，并检查偏移、范围和 glyph ID。
3. 为全部可映射字符生成一个 Microsoft Unicode format-12 子表，添加到内存副本。
   TTC 只提取所选 face。这里不改 glyph ID、轮廓、度量、名称、布局表或原文件；
   仅 cmap 与 SFNT 目录/校验和发生变化。
4. 将副本交给原有 HarfBuzz 子集管线，继续规范化/布局/glyph 闭包以及紧凑重编号。
   不嵌入完整字体兜底。报告的 source SHA256、source bytes 和 face index 仍来自原输入。

已有可用 Unicode cmap 的字体继续使用原字节与 face index，不额外转换。
缺字检查也走相同 CP936 查字逻辑，避免在子集化前误判这类字体为全缺字。

## 数据与兼容范围

24070 对映射编译进原生和 WASM，均为同一纯 Rust 数据查表；没有新增运行库依赖。
源文件、生成方式、SHA256 和 Unicode License V3 见
[映射数据说明](../crates/fonts/data/README.md)。构建不下载或重新生成映射。

这里固定的是 Microsoft CP936 的可逆转换，即 libass Windows 路径的
WC_NO_BEST_FIT_CHARS 语义。系统 iconv 的 CP936 别名/近似转换可能不同；
不能因此承诺所有平台的任意字符都具有同样旧编码行为。仍需在实际固定环境验证。
这不是对任意系统字体或任意播放器的通用保真承诺。

## 本次实际验证

长城粗圆体的 24070 对 CP936 输入使用独立 FreeType 比较：8308 对具有非零字形，
原 format-2 与内存 format-12 的 glyph ID 差异为零。源文件 SHA256 未改变。

原生和真实 WASM 对 s057-0001 的输出逐字节一致；固定 libass 0.17.5、1920×1080、
相同 Noto Sans SC 默认字体、provider NONE 下，3786 个采样时刻零差异。
该字体由 4349438 字节子集化为 69904 字节，消除了之前 882 个差异采样。
使用本次 WASM 默认 warn 重新处理、渲染全部 113 份原缺字样本：454486 个采样零差异。
这不修复默认环境本身也无法显示的字符。

开源合成测试覆盖映射别名、ASCII/DBCS 边界、负 delta/回绕、零 glyph、不可编码字符、
损坏表、TTC face、源表/轮廓/度量保持，以及 native/WASM 子集 golden 一致性。
