# 实验性内存接口

`assfonts-wasm` 将现有 Rust Processor 和 HarfBuzz 暴露为小型 C ABI，供
Emscripten 生成单个 WASM 模块。没有文件系统、CLI 参数或子进程调用；字体和
UTF-8 ASS 由调用者传入，返回 JSON。原生 CLI 仍是 workspace 默认构建目标。

调用顺序：

新引擎默认 warning（`warn`），不自动加载默认字体。可用 `af_set_missing_glyph_policy(engine, 1)` 显式选择 warning，
`0` 恢复严格模式；非法值报错且不改变策略。调用后读取的响应会替换前一次结果。
warning 不加载默认字体，宿主环境约束见 [缺字策略](../../docs/missing-glyph-policy.md)。

1. `af_engine_new()` 创建独立字体 catalog。
2. `af_alloc(len)` 分配输入缓冲区，写入字体/来源名；`af_add_font` 复制并索引
   字体，调用后用 `af_free(ptr, 原始长度)` 释放输入缓冲区。可添加多个文件。
3. 为 UTF-8 字幕分配缓冲区，调用 `af_process`，再释放输入缓冲区。
4. `af_result_ptr` / `af_result_len` 返回借用的 UTF-8 JSON；读取后立即复制。
   添加字体或处理失败返回 0，并提供 `{"error":"..."}`；成功返回 1。
   处理成功的 JSON 含 `subtitle` 和 `report`。普通输入错误不会使 catalog 失效。
5. `af_engine_destroy` 释放 catalog 和响应；不要二次释放或继续使用句柄。

这是受信任宿主适配器使用的内部 ABI，不验证任意地址的合法性。所有裸指针都必须
来自同一模块实例且满足对应 Rust 函数的 Safety 合同。禁止并发使用同一 engine、
跨实例传递指针、使用已失效响应或让输入指向 engine 自己的存储。WASM memory.grow
会使旧 ArrayBuffer 视图失效，宿主每次应重新取 HEAPU8。空输入仍使用 af_alloc(0)
返回的非空指针。panic/OOM/陷阱不属于普通输入错误，宿主应丢弃整个实例。

字体匹配、语法拒绝规则和原生版本相同。内存与输入大小相关，没有在此接口中加入
LRU 或总量限制；销毁 engine 不代表运行时马上向操作系统归还 RSS。

原生 ABI 合同测试（不需要 Emscripten）：

```sh
cargo +1.92.0 test -p assfonts-wasm --locked
```

测试涵盖错误后恢复、字体复制、正文保留和零长度输入缓冲区。WASM 构建与 Deno
运行验证由仓库的构建工具链与独立测试入口提供，不能用原生测试代替实际 WASM 执行。
