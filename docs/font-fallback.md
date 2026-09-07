# libass 缺字行为与一致性边界

本分析以实际回归用的 libass 0.17.5 为准；自动缺字回退尚未实现。当前默认 warn 保留同名候选及缺字报告，显式 error 仍报错；环境约束见 [缺字策略](missing-glyph-policy.md)。2026-09-08 的同分候选修复只对齐载入顺序，不宣称完整字体选择算法已经一致。

## libass 实际做什么

1. 为字体请求建立 ASS_Font，选择初始 face。后续字符先依次查该对象已加载的 faces；因此之前遇到的字符及其回退可能影响之后的选择。
2. 没有字形时，再针对该字符寻找字体。优先在请求的字体族/名字下匹配，检查候选是否覆盖当前字符，字重/斜体最接近者如果不覆盖，也能尝试其他候选；同分保持先注册者。
3. 仍找不到时，依次尝试配置的默认字体族、平台 provider 提供的 fallback family、默认字体路径。这些都是有条件的步骤，并非所有播放器都配置了它们。
4. 部分旧式 charmap 会做符号/编码映射；必要时也尝试其他 charmaps。最终仍找不到时记录日志，字形索引可能保持 0，显示 .notdef（可能是方框，也可能无轮廓），而非让整个字幕处理失败。

依据：[ass_font.c 的 ass_font_get_index / ass_charmap_magic](https://github.com/libass/libass/blob/0.17.5/libass/ass_font.c)、[ass_fontselect.c 的 find_font / ass_font_select](https://github.com/libass/libass/blob/0.17.5/libass/ass_fontselect.c)。

Fontconfig 的 fallback 候选来源于它自己的排序与覆盖集合；CoreText、DirectWrite 的实现及安装字体可能不同。只固定 libass 版本不足以保证跨机器选中同一字体。依据：[Fontconfig provider](https://github.com/libass/libass/blob/0.17.5/libass/ass_fontconfig.c)。

## 已做的最小实验

固定 provider NONE、未配置默认字体族/路径，用仓库 OFL 字体构造主字体仅有 a/c、文本 abc 的场景：

| 额外字体 | 结果 |
| --- | --- |
| 同族 Roboto，覆盖 b | libass 找到额外 face，原字体与内嵌附件渲染一致 |
| 相同字形，名字改成 Unrelated | 不会自动作为 Roboto 的 fallback，4 个可见采样帧均不同 |
| Unrelated 与完全不加回退字体比较 | 渲染一致，证明并未使用这个能覆盖 b 的字体 |

另外，用 Open Sans 的真实 .notdef 轮廓和缺失的 U+0378 测试：仅绕过覆盖报错、继续使用普通子集，在 4 个可见采样帧出现差异。HarfBuzz 默认可丢弃 glyph 0 的轮廓；若要忠实复现“确实无法补字”的源结果，需保留 .notdef 轮廓及度量，而不是把它变成空白。相关开关是 HB_SUBSET_FLAGS_NOTDEF_OUTLINE。此实验在隔离测试字幕上进行，没有放宽正式管线。

## 怎样做到效果一致

推荐先以固定 libass + 固定原字体集合 + 固定添加顺序 + provider NONE 为可复现目标；若目标是某播放器的系统回退效果，还必须取得相同 provider 配置、默认字体和实际 fallback 字体，记录它们的哈希。无法在缺少目标环境时保证任意设备像素一致。

设计上需要将“一个 FontRequest 返回单一 face”改成有序字体计划：主 face、实际回退 face、实际字符/字形需求、未解决字符，以及选择原因。不能把整条对白换成 fallback，也不能只把每个 Unicode 字符独立匹配后忽略组合字符、连字与 shaping 上下文。现有聚合后的 BTreeSet 会丢失字符访问顺序；要精确复现 libass 的 face 缓存行为，需保留有序文本运行段或使用实际 libass 的选择/塑形跟踪作为规划依据。

对每个选中 face 分别执行布局、组件与规范化闭包并子集化，输出中必须让播放器仍能选择到同一 face。不同名字的 fallback 仅作为附件加入并不足够：同族 fallback 可以自然匹配；跨族 fallback 则需要同步播放器的回退配置，或经过验证的字体别名方案。若改写 ASS 的字体标签，也会改变此前的正文不变约束，并可能影响 shaping，因此不能作为无提示的默认修复。

如果完整字体基准本身找不到字形，应区分两种目标：忠实复现原来的 .notdef，或指定新字体让它可读。后者是视觉行为变更，不是“无损兼容”。当前保留显式严格模式；默认 warn 的 cmap 报告不能替代 renderer 的真实回退轨迹。完整回退计划仍属于后续设计。

验证需包含：同族不同字重覆盖不一致、重复名称及加载顺序、组合字符、RTL/连字、竖排、符号 charmap、空格和完全无字形；最终以完整字体真值像素对照及移除 fallback 字体的负对照为准，不以“不再报错”判定成功。
