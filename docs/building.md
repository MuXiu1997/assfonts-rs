# 构建与发布

原生单文件意味着运行者无需另装第三方运行库；各操作系统/架构仍需要独立构建产物。macOS/Windows 保留操作系统库调用。仅 Linux musl 构建目标追求完全静态 ELF。

## macOS（已本机验证）

```sh
git submodule update --init --recursive
cargo build --release --locked
otool -L target/release/assfonts-rs
```

Apple Silicon 本次只有 `/usr/lib/libc++.1.dylib`、`/usr/lib/libiconv.2.dylib`、`/usr/lib/libSystem.B.dylib`。不链接 `/opt/homebrew/...` 下的 HarfBuzz。编译需要 Xcode Command Line Tools；执行不需要。

## Linux musl（cargo-zigbuild 唯一发布入口）

本机和 CI 都执行 `scripts/build_linux.py`，目标固定为 `x86_64-unknown-linux-musl`。工具版本集中在 `mise.toml`，`scripts/linux-toolchain.json` 是供独立脚本和现有 CI 使用的兼容视图：Rust 1.92.0、cargo-zigbuild 0.23.4、Zig 0.14.1。修改版本后运行 `mise run sync-toolchains`。构建脚本校验版本，不自动安装或升级工具。

```sh
rustup toolchain install 1.92.0 --profile minimal --target x86_64-unknown-linux-musl
python3 -m venv .validation/linux-tools
. .validation/linux-tools/bin/activate
python3 -m pip install cargo-zigbuild==0.23.4 ziglang==0.14.1
python3 scripts/build_linux.py
```

输出为 `target/zigbuild/x86_64-unknown-linux-musl/release/assfonts-rs`，同目录生成 `build-manifest.json`，记录工具版本、Git/HarfBuzz revision、工作区是否有改动、Cargo.lock 和二进制 SHA-256。入口固定输出路径与静态链接 Rust flags，拒绝额外 `CARGO_ENCODED_RUSTFLAGS`。可用 `CARGO_ZIGBUILD_ZIG_PATH` 指定同版本 Zig。

HarfBuzz 在 musl 目标使用 Zig 提供的 libc++，由 `crt-static` 选择静态运行库，不混用宿主机 GCC 的 libstdc++。macOS/Windows 的原生配置不变。普通 glibc `cargo build` 不属于 Linux 发布路线，不能将其产物标为完全静态包。

在 Linux 上验证上述文件（不会重新构建）：

```sh
python3 scripts/check_linux_artifact.py \
  --binary target/zigbuild/x86_64-unknown-linux-musl/release/assfonts-rs \
  --output .validation/artifact
```

该检查核对构建清单 SHA-256、ELF 架构、程序头没有 `INTERP`、动态段没有 `NEEDED`，并实际执行后端列表。接着执行 [开源字体渲染回归](../tests/render/README.md)，比较完整原始字体与此二进制生成的附件。

## Windows MSVC（CI 配置，未在本机执行）

需要 Visual Studio C++ Build Tools。PowerShell：

```powershell
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked
dumpbin /DEPENDENTS target/release/assfonts-rs.exe
```

cc crate 会将 Rust 的静态 CRT 选择传给 C++ `/MT` 配置。发布验证应确认没有第三方 HarfBuzz DLL 和需要单独分发的 VC runtime DLL；系统 DLL 仍正常存在。

## WASM（实验性 Emscripten 单模块）

工具版本由 `mise.toml` 同步到 `scripts/wasm-toolchain.json`：Emsdk 4.0.23、Rust
nightly-2025-12-17 和 `wasm32-unknown-emscripten`。HarfBuzz 仍使用同一子模块。
构建入口支持本机 macOS 和 Linux，需要 Python 3、Bash、Git 与 rustup。

```sh
git submodule update --init --recursive
# 显式安装到 .validation/emsdk，并安装指定 Rust 的 rust-src；不改默认 Rust。
python3 scripts/setup_wasm.py
python3 scripts/build_wasm.py
```

两个入口都接受 `--emsdk /path/to/emsdk`，也可使用 `EMSDK` 环境变量。
已有 SDK revision 不匹配会拒绝，不自动覆盖其他版本；构建脚本本身不安装工具。
它校验 Emsdk、emcc、Rust、HarfBuzz 版本并使用 `--locked`，拒绝额外
`CARGO_ENCODED_RUSTFLAGS`。SDK 环境只影响构建子进程，不修改 shell 启动文件。

Rust 标准库通过 `-Zbuild-std=std,panic_abort` 构建，Rust/C++ 共同链接到一个
WASM 模块；关闭 C++ exceptions/RTTI 和 Rust release LTO。不使用 WASI。
输出 `target/wasm/engine.js`、`assfonts-wasm.wasm`、`build-manifest.json` 和
`licenses/`。清单记录版本、源码状态和两个产物的 SHA-256。编译缓存留在
`target/emscripten/`；工具、缓存和生成产物均不提交到 Git。

模块提供纯内存 C ABI，没有文件系统；宿主负责读写字体、字幕及加载 WASM。
配置为 2 MiB 栈和最大 2 GiB 线性内存。普通输入错误返回 JSON；panic/OOM/陷阱
要求宿主丢弃实例。接口合同见 [WASM crate](../crates/wasm/README.md)。
该构建目标不会切换现有 Deno 字幕库，也不改变默认原生 CLI 构建。

## CI

`.github/workflows/ci.yml` 的 macOS、Windows MSVC job 保留原生 Cargo 测试和构建。Linux job 只通过上述入口生成 release 二进制，随后执行静态链接检查、真实开源字体渲染回归及负对照，再次核对 SHA-256 后打包同一文件；不在验证后重新编译。跨平台 Rust 单元测试继续由原生 job 执行。

Linux 渲染依赖仅供独立测试程序使用：libass 固定 0.17.5 并校验源码 SHA-256，FreeType、HarfBuzz、FriBidi 来自运行环境并记录版本。基准与被测组共用同一渲染器版本，但字体环境完全隔离；不同 CI 镜像不要求历史帧哈希相同。字体下载固定上游 commit 和 SHA-256。

成功时上传 `assfonts-rs-linux-musl.tar.gz`，包含经过验证的可执行文件、构建清单和许可文件，tar 保留执行权限。验证报告、字体许可、日志和代表性帧作为独立 artifact 上传，失败时也尽量保留。当前仓库没有配置远程；本地等价验证不能表述为 GitHub Actions 已通过。

本次实际交叉编译、Linux 运行、组合字符修复和真实样本回归的结果见 [2026-09-07 验证记录](validation/2026-09-07-linux-zigbuild.md)。

CI 不自动发布 release。真正对外发布时，应将构建产物与对应第三方许可通知一同分发；HarfBuzz 原始许可保存在 `vendor/harfbuzz/COPYING` 及其子目录。
