# 2026-09-08 固定默认字体 warning 验证

本记录区分默认值修改前的完整渲染实验，以及默认统一为 `warn` 后的新验证。
基线提交为 `bba0cc4`；本轮没有生产替换、部署或推送。
接口与限制见 [缺字策略](../missing-glyph-policy.md)。

## 默认值修改前：显式 warn 的真实语料渲染

此前 113 份缺字拒绝样本，以显式 warn 处理后，112 份成功，1 份因 legacy-only
GBK cmap 明确拒绝。该版 WASM 输出与完整源字体比较：112 份、450700 个采样
时刻、0 个像素差异帧。该数量是此前已完成的渲染实验，本轮没有重跑这些渲染采样。

环境为 libass 0.17.5 / FreeType 2.14.3 / HarfBuzz 14.2.0 / FriBidi 1.0.16，
1920×1080，禁用系统 provider；两侧实际加载相同的 Noto Sans SC 默认字体。
基线与处理 catalog 的注册顺序一致；候选侧不提供原字体，仅子集附件和共同默认字体。
默认字体 SHA-256：`faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9`。

106 份发生默认字体选择；8 份两侧最终仍找不到部分字形，日志次数相同。
像素保真不等于全部文字可读，也不代表全部连续时间或任意播放器验证。

唯一拒绝项 `s057-0001` 的长城粗圆体只有 Microsoft GBK format-2 cmap。
libass 会转码取字；当作普通 cmap 缺字放行的首轮实验有 882 个差异帧。
最终增加 Unicode cmap 前置检查，不再输出这个不保真的结果。
其原因、候选修复和验收条件已单列为 [GBK cmap 后续事项](../known-issues/legacy-gbk-cmap.md)。

上述历史渲染对应的 WASM SHA-256：
`23c56c5bad743822d5fea14f4be72beaf9b64a840814b4a23e80c3683616eee1`。
JS SHA-256：`7546422ce0ef378270ec63f092f29fbd7407fe47ff1b2939a15d0e4f360b0219`。

## 默认 warn 后：本轮实际重跑

使用新构建的 WASM，新建引擎后不调用策略 setter，按原 catalog 注册顺序
重新处理上述 113 份输入：112 份成功且报告策略为 warn，`s057-0001` 仍明确拒绝。
112 份成功输出与上述历史显式 warn 的 `embedded.ass` 逐字节相同。
因此复用其已有渲染证据；此次字节比较不能表述为又完成了 450700 次渲染。

- Rust workspace 39 项测试、格式检查、Clippy（所有 targets、warnings 为错误）、
  无默认后端编译及 native release / WASM 构建通过。
- 实际 Deno WASM Worker 15 项检查通过，包含新引擎默认 warn、显式 warn 等价、
  显式 error 的 TTC/CFF 既有 golden hashes、策略切换、非法值和严格模式恢复；
  TypeScript 检查通过。
- native 和 WASM 严格模式各 50 个 RGBA 采样零差异，负对照校验通过。
  严格渲染 fixture 现在显式选择 error，既有 golden 值未修改。
- 不传策略参数的 native CLI 正对照（默认字体覆盖、默认字体也缺字、保留同名候选）
  各 6 个采样零差异；负对照（移除主字体、默认字体、同名候选）各 6 个采样、
  4 个差异帧。对照固定 provider 为 NONE，使用同一 Open Sans 默认字体。

新 WASM SHA-256：`0b73eff8329409ac89a36da088bee15055d9ea093693d4b47119ff814f6952e9`。
新 JS SHA-256：`7546422ce0ef378270ec63f092f29fbd7407fe47ff1b2939a15d0e4f360b0219`。
子集 HarfBuzz 固定为 14.4.0，独立于上述历史渲染器使用的 14.2.0。
本轮未重新验证此前全部 1014 份严格模式真实语料。
