# 待修复：只有 GBK cmap 的字体

状态：独立后续事项。本次 warning 策略提交仅记录问题，未实现转码映射；
legacy-only cmap 的拒绝保护继续生效。

## 原因与复现证据

真实语料 `s057-0001` 使用的长城粗圆体只有 Microsoft platform 3、encoding 3、
format 2 的 GBK cmap，没有 Unicode cmap。libass 可先将 Unicode 转为 CP936
编码，再从旧 cmap 获取 glyph；目前的 Unicode 子集路径不能直接复现该映射。
这不是字体损坏，也不能当作通常的 Unicode cmap 缺字交给默认字体处理。

最初直接放行缺字的实验产生 882 个差异帧，因而增加 Unicode cmap 前置检查。
当前默认 warn 仍明确拒绝该样本，不输出替代字体或完整字体绕过错误。
其余真实语料和实验环境见 [验证记录](../validation/2026-09-08-warning-policy.md)。

## 候选修复方案

在内存中建立 Unicode → CP936 → 原 glyph ID 映射，并为子集流程提供等价的
Unicode cmap，随后执行既有 glyph/layout 闭包和正常子集化。需要先验证
FreeType/libass 实际映射规则、format 2 读取及多码点别名，不预先假定方案保真。

native 与 WASM 应共用可移植、确定性的编码映射，不依赖系统 iconv 或宿主编码库。
不修改原始字体文件、ASS 字符或字体名称，不嵌入完整字体兜底。
该方案尚未在本提交中实现或验证；也不代表 Big5、其他旧编码或缺失 cmap 已受支持。

## 验收条件

1. 为 format 2、ASCII/双字节边界、不可编码字符、缺失 glyph、损坏表和映射别名
   增加有意义的测试，验证保留 glyph、轮廓、度量及布局闭包。
2. native 与真实 WASM 使用相同输入得到一致映射和子集结果，既有严格 golden、
   warning、规范化、`.notdef`、候选顺序及布局兼容测试继续通过。
3. 对 `s057-0001` 在相同完整源字体、注册顺序、默认字体及固定渲染器环境中重跑
   全部既定采样，要求零像素差异，并检查默认字体选择与缺字日志。
4. 完成以上验证后，只放行已支持并验证的 GBK 情形；其他 legacy-only cmap
   或无法可靠映射的情况继续明确拒绝。
