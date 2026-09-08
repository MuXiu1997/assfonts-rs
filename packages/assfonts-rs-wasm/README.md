# @muxiu1997/assfonts-rs-wasm

ASS 字体子集化和内嵌的纯内存 ESM 库，使用 Rust 和静态 HarfBuzz。
包内包含预编译 WASM、JS 和 TypeScript 类型；没有运行时 npm 依赖、安装脚本或运行时下载。

## 支持范围

首版验证环境为 Deno 2.5.6 的 module Worker。Node、浏览器、主线程初始化和 bundler
接入尚未作为支持范围验证。默认加载器需要读取包内文件的权限。
安装/缓存依赖后，处理本身可以使用 `--deny-net --deny-run`。

```ts
// worker.ts
import { createEngine } from 'npm:@muxiu1997/assfonts-rs-wasm@0.1.0-beta.0'

const engine = await createEngine({
  parseMode: 'strict',
  missingGlyphPolicy: 'warn',
})
try {
  engine.addFont('Example.ttf', await Deno.readFile('./Example.ttf'))
  const result = engine.process(await Deno.readFile('./subtitle.ass'))
  await Deno.writeTextFile('./output.ass', result.subtitle)
  postMessage({ report: result.report })
} finally {
  engine.close()
}
```

宿主通过 `new Worker(new URL('./worker.ts', import.meta.url).href, { type: 'module' })`
启动 Worker，处理完成或出错后调用 `worker.terminate()`；生产宿主还应设置超时并处理
`onerror`。库不负责文件扫描、解压、输出命名、Worker 队列或批处理。

## API 与生命周期

- `await createEngine(options?)`：独立模块和字体 catalog；默认 `strict` / `warn`。
  可传 `wasmBinary: Uint8Array` 替换包内二进制，要求与当前包 ABI 一致，初始化前会复制。
- `addFont(label, bytes)`：复制字体并返回 `{ faces }`（catalog face 总数）。
  相同字节去重，保留第一次来源名和注册顺序。同分候选先注册者优先。
- `process(bytes)`：同步处理 UTF-8 ASS v4+，返回 `{ subtitle, report }`。
  字幕原文保留，仅插入字体附件；report 的 snake_case 字段与 Rust JSON 一致。
- `setParseMode('strict' | 'compatible')` / `setMissingGlyphPolicy('error' | 'warn')`。
- `close()`：幂等释放 catalog；关闭后不能继续调用。每个实例顺序使用。

`AssfontsError.code` 为 `INPUT`、`CLOSED`、`FATAL` 或 `INITIALIZATION`。
普通 `INPUT` 错误可继续使用；`FATAL` 后应终止 Worker，不能继续调用 guest 清理。
初始化失败不返回 engine。不要按错误文本判断可恢复性。

同字体集复用 engine，字体集变化时关闭并重建；批次结束回收 Worker。
同步处理不能在同一线程被 AbortSignal 中断；需要中断时由宿主终止 Worker。
默认最大线性内存 2 GiB、栈 2 MiB，不代表 RSS 上限。close/terminate 不保证 OS 立即回收 RSS。

支持 TTF/OTF/TTC/OTC。拒绝非 UTF-8、SSA/v4++、已有 `[Fonts]` 等不支持输入；
不自动加载系统字体，也不支持 WOFF/WOFF2 解码。缺字 warn 依赖宿主保持相同的字体回退环境，
不是补齐缺失字形。完整兼容范围见仓库 README 和 docs。

## 开发与发布

仓库根目录执行：

```sh
mise run build:wasm
npm ci --prefix packages/assfonts-rs-wasm
python scripts/package_wasm.py
python scripts/test_npm_wasm.py
```

`package_wasm.py` 校验已有 WASM 的哈希和源码清单，不重建 WASM。
`test_npm_wasm.py` 在独立临时项目安装 `target/npm/*.tgz`，执行类型检查及离线 Worker 验收。
完整核心/渲染验证使用 `mise run test:wasm`；发布 CI 先执行该验证，再打包同一份产物。

修改本目录 package.json 版本，创建匹配的 `wasm-v<version>` tag 可触发发布。
CI 仅接受 `X.Y.Z-beta.N` 版本并显式使用 `--tag beta`。
`0.1.0-beta.0` 已通过本地验证后的 tarball 完成首次公开发布；后续使用 beta.1、beta.2 等递增，
例如 `wasm-v0.1.0-beta.1`。不要重新发布已经存在的版本。
发布 workflow 使用 npm trusted publishing：包设置已关联
`MuXiu1997/assfonts-rs`、workflow `release-wasm.yml` 和 environment `npm`。
当前仍处于 beta 阶段，不作稳定版承诺。使用者应显式指定 `@beta` 或完整 beta 版本。
本地构建、推送普通分支不会发布。产物和依赖目录不提交到 Git。
