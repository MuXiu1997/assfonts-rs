# mise 开发入口

工具版本、任务环境和开发命令统一定义在仓库根目录的 `mise.toml`。本轮只接入
项目任务；现有 `.github/workflows/` 没有迁移，GitHub Actions 集成另行处理。

## 开始使用

需要 mise 2026.8.15 或更新版本。首次使用：

```sh
mise trust
mise install
mise run check
mise run test:native
```

`mise run` 和 `mise exec -- COMMAND` 会加载项目环境，不需要全局修改 PATH 或
激活 shell。原生任务默认使用 Rust 1.92.0；WASM 脚本显式调用固定日期 nightly，
mise 负责安装其 rust-src。Rust target、Clippy 和 rustfmt 也在配置中声明。
Cargo backend 安装 cargo-zigbuild，Zig 单独管理，不额外安装 Python ziglang 依赖。

已有同版本工具可用 `mise link TOOL@VERSION /installation/root` 纳入管理；这是本机
状态，不把绝对安装路径提交到仓库。SDK 和系统编译依赖仍需下面的准备步骤。

## 任务

| 命令 | 行为 |
| --- | --- |
| `mise run check` | 版本清单一致性、Rust fmt/Clippy、可选后端、Actionlint、Deno 类型检查 |
| `mise run test:native` | 完整 Rust workspace 单元和集成测试 |
| `mise run build:native` | 原生 release CLI |
| `mise run build:linux` | 通过既有脚本构建固定版本 musl release |
| `mise run setup:wasm` | 校验并安装固定 revision 的 Emsdk；安装所需 Rust 源码组件 |
| `mise run build:wasm` | 通过既有脚本构建 WASM；不会自动安装 SDK |
| `mise run setup:fonts` | uv 隔离环境中准备固定来源的开源字体 |
| `mise run setup:renderer` | 构建独立 libass 渲染器，需要系统开发库 |
| `mise run test:wasm` | 对已有 WASM 执行 Deno 和 RGBA/负对照验证 |
| `mise run test:linux` | 仅 Linux：对已有 ELF 执行静态检查和渲染验证 |
| `mise run sync-toolchains` | 从 mise 版本配置刷新独立脚本/旧 CI 使用的 JSON 视图 |

构建任务与验证任务分开，保证验证和后续打包可以使用同一份产物。渲染任务每轮创建
新的 `.validation/mise-*` 目录，不覆盖旧报告；缺少产物、字体或渲染器时会明确提示
先运行哪个准备任务。`test:linux` 不会在 macOS 上假装验证 Linux ELF。

```sh
mise run setup:wasm
mise run build:wasm
mise run setup:fonts
mise run setup:renderer
mise run test:wasm
```

## 环境和版本边界

- `EMSDK` 默认指向项目的 `.validation/emsdk`，允许调用者设置为已有 SDK 根目录。
  SDK revision 和 emcc 版本仍由构建脚本核验，不在本轮切换 Emsdk 分发方式。
- Python 字节码缓存定向到 `.validation/pycache`。fontTools 使用已有
  `tests/render/requirements.txt` 和 uv 临时隔离环境，不污染全局 Python。
- FreeType、FriBidi、pkg-config、C/C++ 编译器等系统开发依赖继续按
  [渲染说明](../tests/render/README.md) 安装；mise 不会自动运行 sudo、brew 或 apt。
- `mise.toml` 是工具版本的权威来源。两个 JSON 清单保留目标、revision 校验等
  构建元数据，版本字段由 `sync-toolchains` 更新；`check` 和跨平台构建入口先检查
  两者一致性。升级 Emsdk/Rust 时还必须复核其 revision/编译器 hash，不能只改版本号。
- `Cargo.lock`、字体校验和 Python 依赖仍属于各自的依赖层，不由 mise 取代。
  提交 `mise.lock` 中工具后端提供的解析版本、下载地址和校验信息；它不保证锁定
  Cargo backend 的全部编译依赖或操作系统包。不要手改锁文件，用 `mise lock` 更新。
  不提交 SDK、工具安装目录、缓存、个人路径或生成的字体/二进制。
