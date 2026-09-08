# WASM 内存增长调查与修复 · 2026-09-08

本次复现没有发现同负载反复处理导致 WASM 活跃分配持续增长。发现并修复了重复字体的无效拷贝、响应与附件的瞬时副本及旧响应滞留；三个配对负载的线性内存峰值下降 **15.3%–21.8%**，处理耗时基本持平。普通负载的进程 RSS 基本不变，因此不能把这些修复描述为解决了所有 RSS 增长。

所有工作位于独立 worktree，分支 `codex/wasm-memory-growth`，基线为 `bba0cc459672d33eddcc87672a2136e3cb414716`。本记录描述提交整理前完成的实验；实验完成时改动未提交、未合并、未推送，其他仓库未修改。机器可读结果见 [汇总 JSON](2026-09-08-wasm-memory-summary.json)。私有原始证据位于本 worktree 的 `.validation/memory/`，其中的字体路径和商业字体不进入版本控制。

## 测量口径与复现范围

环境：Apple M1 Pro（10 核）、32 GiB RAM、macOS 26.6.2、Deno 2.5.6、Emscripten 4.0.23、Rust nightly-2025-12-17。构建使用固定 HarfBuzz `36cb489cb02ce4b92099669ba9f9bea348eff93f`。基线仅添加诊断导出，补丁见 `tests/wasm/baseline-memory.patch`；优化版使用同样的诊断配置。主对比使用最终同一套宿主测试工具、冻结的构建产物，串行执行且不与编译重叠。普通发布产物另行执行 ABI 和保真验证。

| 指标 | 采样方式 | 可以说明什么／限制 |
| --- | --- | --- |
| WASM 线性容量 | 每个生命周期阶段读取 HEAPU8.buffer.byteLength | 已增长的容量，不等于活跃对象或物理驻留；释放对象不会缩小它 |
| guest 活跃分配 | dlmalloc mallinfo，每次加载、处理、释放和销毁采样 | 包含分配器开销；阶段峰值是瞬时峰值的下界 |
| 瞬时请求分配峰值 | 单独启用 Emscripten malloc/free/realloc hooks | 记录每次申请，排除 trace 运行的性能数据；全部 trace 无未知 free 或重复 allocation |
| JS 堆和 external | Deno.memoryUsage、V8 heap statistics | external 包含 WASM buffer，不能再加在线性容量上；GC 时机影响读数 |
| 进程 RSS | 父线程约 20 ms、独立 ps 约 50 ms 采样 | 同进程所有 Worker 共享；是采样最大值，可能遗漏极短峰值 |
| 虚拟地址／驻留区 | ps VSZ 和独立 vmmap 探针 | 虚拟预留不是物理内存；不同工具的记账方式不能直接相减 |

每个 Worker 最长 120 秒，外层进程最长 600 秒，RSS 守卫 4 GiB，并发最多 2。守卫为采样后的终止措施，并非操作系统硬内存限制。没有实际尝试占满 2 GiB WASM 或整机内存。默认对比不强制 GC；GC、allocation trace、vmmap、SAFE_HEAP 都是独立诊断实验。

覆盖冷启动、逐字体加载、第一次处理、30/100 轮复用、同目录多个字幕、重复添加大 TTC、目录切换、输入与结果释放、engine 销毁、Worker 终止重建、错误恢复，以及两个 Worker 的受控并发。配置保存原始字体注册顺序，以内容 SHA-256 验证输入。

| 负载 | catalog 字节 | 文件／face | ASS 输入字节 | JSON 结果字节 |
| --- | ---: | ---: | ---: | ---: |
| 大输出 | 92,730,048 | 9／9 | 4,817,751 | 8,306,822 |
| 大 catalog | 447,296,148 | 34／34 | 82,356 | 2,526,875 |
| 大 TTC | 301,401,364 | 5／78 | 76,028 | 1,671,318 |

最大单字体为 162,590,364 字节；TTC 的多个 face 共享 Arc 字节存储，没有按 face 数复制整份文件。

## 配对结果

以下容量和 RSS 单位均为 MiB（2²⁰ 字节）。大输出和大 catalog 各为两个 Worker 生命周期、每个 30 次处理；重复 TTC 为最大字体额外添加 10 次、随后处理 3 次，因此其处理耗时不宜解释为精确性能变化。原始文件为 `paired-{baseline,optimized}-*.jsonl` 及同前缀 `.process.jsonl`、`.environment.json`。

| 负载 | 线性容量：前 → 后 | 降幅 | RSS 峰值：前 → 后 | 处理中位数：前 → 后 |
| --- | ---: | ---: | ---: | ---: |
| 大输出 | 166.81 → 139.00 | 16.7% | 657.84 → 658.33 | 298.69 → 298.24 ms |
| 大 catalog | 650.25 → 550.63 | 15.3% | 1176.91 → 1175.98 | 324.99 → 324.46 ms |
| 重复大 TTC | 712.06 → 557.00 | 21.8% | 1535.66 → 1357.22 | 937.67 → 941.69 ms |

独立 allocation trace 显示，大输出的瞬时请求分配峰值从 156,724,917 降到 126,515,202 字节（149.46 → 120.65 MiB，下降 19.3%）。重复 TTC 从 626,606,292 降到 464,016,248 字节，差值约为一份最大字体。这支持“消除瞬时副本”的因果解释，而不只是改变了扩容余量。销毁后 trace 剩余 3 个模块级分配、253 请求字节，对应 mallinfo 280 字节。

大输出同 engine 连续 100 次运行，优化前后尾部 10 次的线性容量和活跃分配斜率均为 0 字节／次。优化版不 clear 时稳态为 102,258,336 字节，调用新增的 `af_result_clear` 后为 92,735,128 字节（接近 catalog 本体），销毁 engine 后为 280 字节。不能由这些有限负载证明任意输入都绝无泄漏。

## 根因分类与实现

| 来源 | 证据与结论 | 处理 |
| --- | --- | --- |
| 必要输入及算法工作区 | catalog 在 engine 生命周期内持有字体；处理同时需要输入、HarfBuzz 子集工作区、输出 | 保留算法和保真要求；不能以丢字体或弱化子集规则节省内存 |
| 重复字体无效拷贝 | 原实现先构造 Arc 再去重，重复大 TTC 多出约一份 155 MiB 字体 | 新增 `FontCatalog::add_slice`，对借用字节先散列，命中后不复制；首次来源和注册顺序不变 |
| 响应、附件临时副本 | 旧响应与下一轮处理重叠，Value 序列化复制字符串，附件编码有中间字符串 | add/process 开始先释放旧响应，借用 typed response 序列化，序列化前释放二进制附件，编码直接写入最终字符串 |
| 最后一个响应滞留 | 原 ABI 保留最后 JSON 至下次调用或 destroy | 新增幂等 `af_result_clear`，参考适配器解码后 finally 清理，保留 catalog |
| guest 高水位与空闲块布局 | live 回落、capacity 不回落；增长参数和 staging 实验改变容量 | 释放内存供实例复用；需要释放整个实例时终止 Worker |
| JS 结果引用 | 保留完整结果的正对照能产生显著 heap 增长 | 释放全文引用、仅保存路径和状态；实际历史 BatchResult 已只存路径／状态，不能归咎于它 |
| 宿主原生分配器驻留空闲页 | vmmap 在 Worker 终止后显示大量 empty malloc 区驻留 | 解释 RSS 为何不立即回落；不能靠 guest clear 保证归还全部 RSS |
| 真实持续泄漏 | 测试范围内 guest live 无正斜率，反复 destroy 回到相同基数 | 未观察到；保留更多输入和其他运行时上的验证边界 |

参考宿主另外修复了 `memory.grow` 导致输入视图 detached 后长度改变的问题：分配前保存长度，清理使用原长度。实际 WASM 测试强制触发分离，确认异常后的有效输入仍能正常处理。guest trap 会污染整个模块，适配器拒绝继续调用和新建 engine，且不再尝试 guest 清理，随后销毁 Worker。

这些改动没有改变字体选择、子集算法、整字体回退条件或附加字体字节。typed JSON 的属性顺序可能改变，语义保持一致；不承诺原始 JSON 文本逐字节一致。

## RSS、GC 与重建

10 次简单 Worker 重建的终止后 RSS 落在约 359–468 MiB，未呈持续单调增长。更强的混合测试使用 10 个 Worker 生命周期、每个 25 个任务／8 个 catalog：250 次成功，80 次 engine 销毁全部回到 280 字节。峰值 RSS 为 1623.59 MiB，终止后 RSS 依次约 1008、1028、831、1034、1034、1031、836、612、621、591 MiB。复杂负载的宿主高水位远高于简单负载，不能用单一约 450 MiB 平台外推所有任务。

JS 保留正对照中，30 个完整解码结果在诊断 GC 后仍占 heapUsed 473,256,776 字节；清空引用但不 GC 时仍约相同，GC 后降至 7,306,088 字节，guest live 一直为 280。仅存 30 个路径／状态对象约 7.3 MB。历史宿主的 BatchResult 数组属于后一种，因此本次没有证据把其历史 RSS 归因于完整字幕引用滞留。

独立 vmmap 探针在 Worker 终止后显示 DefaultMallocZone 实际 allocated 仅约 3236 KiB，而 resident 274.4 MiB；dirty fragmentation 为 122 MiB（98%）。`MALLOC_SMALL (empty)` 仍 resident 253.3 MiB，说明“对象已释放、原生分配器空闲区仍驻留”确实发生。Memory Tag 255 的虚拟大小从 34.9 GiB 到 32.6 GiB，而 resident 从 166.8 MiB 到 15.4 MiB；不能把整个 tag 精确指认为 WASM。其他 ps 样本约 483 GB VSZ 同样不是物理用量。vmmap resident、physical footprint 和 ps RSS 口径不同，报告没有将它们互相相减来估算 JS 内存。

这能够解释本机复现中的一部分 RSS 高水位，不能事后精确归因旧 Linux 环境的 2.44 GiB RSS，也不能保证其他 Deno／OS 版本表现相同。

## 参数与运行策略

默认保留 dlmalloc、20% 几何增长、2 GiB 线性上限。固定版本的 Emscripten 设置见 [4.0.23 settings.js](https://raw.githubusercontent.com/emscripten-core/emscripten/4.0.23/src/settings.js)。WASM 扩容还会使既有非共享 buffer 视图失效，宿主需重新取视图，见 [Memory.grow](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Memory/grow)。

| 优化版变体 | 大输出线性 MiB | 大 catalog 线性 MiB | 重复 TTC 线性 MiB |
| --- | ---: | ---: | ---: |
| 默认 | 139.00 | 550.63 | 557.00 |
| growth-step=0 | 133.69 | 527.50 | 557.00 |
| emmalloc | 139.00 | 550.63 | 557.00 |
| 复用最大字体输入缓冲区 | 126.38 | 486.94 | 521.63 |
| 复用输入 + growth-step=0 | 123.44 | 463.38 | 444.69 |

各变体字幕 SHA 相同。growth-step=0 的普通处理中位数约慢 1%–3%，也可能包含运行噪声；更换 allocator 没有明显收益。emmalloc 的 mallinfo 包含 region header，销毁后 332 字节不应与 dlmalloc 的 280 字节作净对象比较。复用输入缓冲区改善分配布局，但加载期间保留最大 scratch，不能解释为所有阶段的活跃峰值都更低。该策略仅实现于测试宿主的 `reuseFontInput`，没有擅自修改其他仓库的生产宿主。

运行建议为复用同 catalog、及时 clear 结果、目录切换 destroy engine，并默认串行。只有两个小负载 Worker 的并发经过验证，不能直接把 2 GiB 上限乘以任意并发数。按输入大小和实际 RSS 留出预算，在批次边界、资源预算超出或 fatal trap 后回收 Worker；如需保证整个进程的原生 allocator 驻留释放，可由外部监督进程在批次边界退出进程，该方案本次未实现。

不建议每个字幕重建 Worker：大输出 catalog 加载中位数约 779.8 ms、模块工厂约 5.4 ms。100 次同 Worker 总计 33.431 秒，10×10 次 Worker 总计 44.528 秒，后者包含每次 250 ms 人工空闲及采样开销；多出的 9 次 catalog 加载本身约 7 秒。仅凭 catalog 总字节 S 和最大文件 M 不能给出严格内存上界，输出、子集复杂度和分配布局仍有影响。

## OOM 与安全边界

受控 32 MiB 最大线性内存构建验证：`af_alloc` 对 3 GiB 无效布局和 64 MiB 超限请求返回 0，随后正常处理成功。加入约 20 MiB 的有效填充字体触发 Rust Arc 所有权分配失败，报 `memory allocation of 20971528 bytes failed` 并以 `RuntimeError: unreachable` trap；这不是已证明的越界访问。trap 后 guest 清理调用为 0，同模块新 engine 被拒绝，新 Worker 能生成相同结果 SHA。

因此“输入 malloc 能返回空”不代表 Rust/HarfBuzz 内部所有分配都可恢复。正常 JSON 错误可复用 engine；OOM、panic、trap 必须丢弃模块。2 GiB 是构建上限，不是可用 RAM 保证，也不是本次压力测试规模。裸 C ABI 仍要求受信任指针及调用顺序，不能对任意恶意指针提供安全保证。

SAFE_HEAP=2（允许 WASM 合法非对齐访问）、ASSERTIONS=2、STACK_OVERFLOW_CHECK=2 的独立构建通过 ABI 与代表样例验证。Deno Worker 的 Node 兼容全局 `process` 会触发 Emscripten 的环境误判，测试加载器仅在模块初始化期间临时屏蔽并恢复其属性；没有关闭 guest 安全检查，也没有使用原生 fallback。这些验证不构成任意输入下内存安全的形式化证明。

## 回归与保真

- workspace 单元测试、无默认特性测试、Clippy、Rust fmt、工具链同步检查、actionlint 和 Deno 类型检查均通过，日志保留于原始证据目录。
- 新增字体借用去重／所有权测试、附件编码边界测试、clear 幂等测试、实际 WASM 重复字体不扩容测试、detached 输入恢复和 fatal module 拒绝测试。
- 普通发布构建与 SAFE_HEAP 构建分别对 25 个历史渲染已验证样例进行独立 SHA 和完整 ASS 字节比对，全部一致，共 151 个附件；涵盖全部 17 个 BASE／注册顺序修复样例及 8 个大内存／mort 等代表样例。没有将其表述为重新渲染完整历史语料。
- 普通发布 WASM 的当前 libass 回归比较 50 帧 premultiplied RGBA，全部 0 差异像素，系统字体 provider 为 NONE，无字体／缺字体负对照有效。证据：`.validation/mise-wasm-s7jkba76/render/{wasm-check,render}.json`。
- 最终普通 WASM SHA-256：`1356da926265a1551c4b565eb064c223c93eb2a02eb52a05d22ccc5cb7543700`；JS：`e89b42b63732a1083041cdc88d9e88f707e7db496b15a0b73f7ce239f8632b35`。诊断构建与普通产物的哈希不同，见 JSON 和各 build manifest。

## 复跑命令与证据定位

从本 worktree 根目录运行。`EVIDENCE` 指向已有完整回归证据目录（含 `cases`、`catalogs-portable`、`work/rejection-validation`）；本机原始位置记录于私有配置。`EMSDK` 使用已固定的本地 SDK。本节命令中的变量需替换为对应目录；不要覆盖已有运行前缀，runner 使用独占创建。

```sh
python scripts/build_wasm.py --emsdk "$EMSDK" --memory-profile
python scripts/prepare_memory_benchmark.py "$EVIDENCE" --output .validation/repro-memory --build target/wasm-memory
python scripts/run_memory_benchmark.py .validation/repro-memory/large-output.json .validation/repro-memory/run-large-output
python scripts/summarize_memory_benchmark.py .validation/repro-memory/run-large-output.jsonl

# 本次所有实验数据重新生成公开汇总（同时断言完成数量、SHA、trace 和 parity）
python scripts/compare_memory_benchmarks.py .validation/memory > .validation/memory/recomputed-summary.json

# 当前普通发布构建与历史代表样例逐字节比较
python scripts/build_wasm.py --emsdk "$EMSDK"
python scripts/prepare_memory_parity.py "$EVIDENCE" target/wasm .validation/repro-memory/parity.json
python scripts/run_memory_benchmark.py .validation/repro-memory/parity.json .validation/repro-memory/run-parity
```

基线重建应在另一个隔离源码目录检出上述基线 SHA，应用 `tests/wasm/baseline-memory.patch`，初始化固定 submodule，再使用相同 SDK 和 `--memory-profile`；不要在当前工作树回退源码。冻结前后构建目录分别为 `baseline-build`、`optimized-build`，最终普通构建为 `final-build`。

合并到默认 warn 的主分支后，两个 prepare 工具会显式写入 `missingGlyphPolicy: "error"`，
以保持本记录的历史严格模式比较口径；旧配置重跑时也应补充该字段。
测试宿主兼容没有策略 setter 的历史严格模式基线。省略该字段仍使用被测构建的默认策略，
不会改变产品的默认 warn。

其他变体构建参数：`--memory-profile --allocation-trace`、`--memory-profile --safe-heap`、`--memory-profile --growth-step 0`、`--memory-profile --allocator emmalloc`、`--memory-profile --maximum-memory 33554432`。每个变体构建完须复制产物到独立目录，再把配置 `build` 指向它，避免后一次编译覆盖前一次证据。32 MiB 限制构建可用 `deno run --allow-read --allow-write --deny-run --deny-net tests/wasm/limits.ts BUILD OUTPUT.json` 验证。

`.validation/memory/README.md` 索引全部原始实验，配置中的 `loops`、`restarts`、`cycles`、`duplicates`、`concurrency`、`gc`、`clearResults`、`reuseFontInput`、`retainResults` 控制独立变量；GC 实验另外传 `--expose-gc`。输入路径和原始配置保持私有；公开 JSON 仅含计数、字节、哈希和匿名负载名。环境文件记录命令、源文件哈希、PID、退出码和采样方式，构建 manifest 记录工具链、参数和产物哈希。复跑需要原始私有语料，仓库自身的公开 ABI 测试不依赖这些商业字体。
