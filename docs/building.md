# 构建与发布

原生单文件意味着运行者无需另装第三方运行库；各操作系统/架构仍需要独立构建产物。macOS/Windows 保留操作系统库调用。仅 Linux musl 构建目标追求完全静态 ELF。

## macOS（已本机验证）

```sh
git submodule update --init --recursive
cargo build --release --locked
otool -L target/release/assfonts-rs
```

Apple Silicon 本次只有 `/usr/lib/libc++.1.dylib`、`/usr/lib/libiconv.2.dylib`、`/usr/lib/libSystem.B.dylib`。不链接 `/opt/homebrew/...` 下的 HarfBuzz。编译需要 Xcode Command Line Tools；执行不需要。

## Linux musl（CI 配置，未在本机执行）

建议在原生 musl 环境构建，避免混用 glibc 版 C++ 静态库。例如在 Alpine 的 Rust 工具链环境：

```sh
apk add --no-cache g++ git binutils
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --locked --target x86_64-unknown-linux-musl
readelf -l target/x86_64-unknown-linux-musl/release/assfonts-rs
readelf -d target/x86_64-unknown-linux-musl/release/assfonts-rs
```

程序头不能包含 `INTERP`，动态段不能包含 `NEEDED`。HarfBuzz build.rs 在 musl 目标静态链接 `libstdc++`。普通 glibc `cargo build` 仍静态链接 HarfBuzz，但可能动态依赖 libc/libstdc++；不能将其标为完全静态包。

## Windows MSVC（CI 配置，未在本机执行）

需要 Visual Studio C++ Build Tools。PowerShell：

```powershell
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked
dumpbin /DEPENDENTS target/release/assfonts-rs.exe
```

cc crate 会将 Rust 的静态 CRT 选择传给 C++ `/MT` 配置。发布验证应确认没有第三方 HarfBuzz DLL 和需要单独分发的 VC runtime DLL；系统 DLL 仍正常存在。

## CI

`.github/workflows/ci.yml` 配置 macOS、Windows MSVC 和 Linux musl 的测试、release 构建与产物上传。只有 macOS 本次在本地实际验证；增加 CI 配置不等于其他平台已经通过。当前项目尚未创建远程仓库或触发远程构建。

CI 不自动发布 release。真正对外发布时，应将构建产物与对应第三方许可通知一同分发；HarfBuzz 原始许可保存在 `vendor/harfbuzz/COPYING` 及其子目录。
