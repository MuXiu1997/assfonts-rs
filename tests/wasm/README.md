# 实验性 WASM 构建与验证

从仓库根目录运行。构建只生成单模块 WASM 与 ES module 加载代码；本目录是独立
验证宿主，不是已发布的 ass-processor-lib 集成。公开批处理 API 的替换仍留在原型中。

## 可重复入口

先按 [构建说明](../../docs/building.md) 运行 `scripts/setup_wasm.py` 和
`scripts/build_wasm.py`。Emsdk、Rust、HarfBuzz 与 Deno 的版本固定于
`scripts/wasm-toolchain.json`；SDK 不进入 Git。

```sh
python3 -m venv .validation/render-tools
. .validation/render-tools/bin/activate
python3 -m pip install -r tests/render/requirements.txt
python3 tests/render/prepare_fonts.py --output .validation/ci-fonts
deno check tests/wasm/verify.ts tests/wasm/worker.ts
deno run --allow-read --allow-write --deny-run --deny-net --no-remote \
  tests/wasm/verify.ts .validation/ci-fonts .validation/wasm-render
```

输出目录必须不存在，避免旧报告混入本轮。验证器使用本地文件和 Web Worker，
没有 npm/jsr 依赖，不允许子进程或网络回退。执行前后核对构建清单中 JS/WASM
的 SHA-256。断言失败或 Worker 异常会返回非零退出码，并终止 Worker。

覆盖 UTF-8 拒绝、缺字体拒绝、损坏字体后恢复、输入字体的独立副本、TTC face 1、
CFF、正文原样保留、重复处理、关闭后拒绝。还强制一次 memory.grow，随后重新处理，
检查宿主没有使用失效 HEAPU8 视图。子集 SHA-256 与已独立渲染验证的原生基准匹配；
固定的是测试输入及其正确输出，不把文件路径或运行时间作为断言。

## 独立渲染

按 [渲染测试说明](../render/README.md) 安装测试用的 FreeType、HarfBuzz、FriBidi，
然后使用与 Linux 发布验证相同的固定 libass 0.17.5 渲染器：

```sh
sh tests/render/build_oracle.sh
.validation/oracle/verify_libass .validation/wasm-render \
  .validation/ci-fonts/NotoSans.ttc .validation/ci-fonts/NotoSansSC-Regular.otf \
  > .validation/wasm-render/render.json
```

50 个时刻比较完整字体与 WASM 内嵌子集的预乘 RGBA，关闭系统字体 provider。
无附件、只留 TTC、只留 CFF 的负对照证明结果确实依赖相应内嵌字体。
渲染器属于独立测试工具，不是 WASM 的依赖。CI 使用固定版本 Deno 2.5.6 执行以上
实际产物，然后打包同一份文件；不在测试后重建。仅上传实验 artifact，不自动发布。

## 已知边界

- 2 MiB 栈、最大 2 GiB 线性内存；OOM/陷阱未做穷尽验证，不应继续复用损坏实例。
- 之前的原型批次测试中，15 轮新建/关闭 Worker、共 180 次处理后，关闭后 RSS
  从约 356 MiB 增至约 684 MiB，不能断言长期常驻内存稳定。释放 catalog 或关闭
  Worker 不等于进程立即把 RSS 归还给操作系统。本提交不宣称修复此问题。
- 语法范围沿用原生解析器，例如 `\fa` 仍拒绝；不忽略错误或改写原始字幕来通过测试。
- 测试宿主按 Deno 设计；没有验证浏览器接入或原生 Windows 上的 Emscripten 构建。
- 既有原型的 43 个真实字幕、143445 个采样时刻零差异是历史补充证据，不是这次
  新产物重新运行的全部样本。本次重建验证记录见
  [2026-09-07 WASM 记录](../../docs/validation/2026-09-07-wasm.md)。
