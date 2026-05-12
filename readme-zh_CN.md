# Mimalloc Replace

[English](./readme.md) | 中文

`mimalloc-replace` 是一个动态劫持了 Windows UCRT 默认分配器的注入模块，它将对应调用转发到内置的 mimalloc 分配器，并利用重分配函数将 UCRT 堆内的活跃数据迁移至 mimalloc 堆。

## 项目需求

mimalloc 本身提供了`mimalloc-redirect.dll`和`minject.exe`，可以通过修改程序 IAT 让 mimalloc 在启动时完成分配函数的替换。一般来说这种做法已经足够高效，但对于游戏等更新频繁且对注入便利性要求极高的程序而言，IAT 修改的方式就不够灵活了。

采用 DLL 注入结合内联 Hook 的方法虽然能便捷地接管分配器，但它又引入了如何处理 Hook 前已经分配的存量内存的问题。

如果直接将 `free` 或 `realloc` 路由到 mimalloc，会导致 mimalloc 试图释放 UCRT 分配的内存，从而引发崩溃。

常见的做法是维护双堆机制（判断指针属于哪个堆再调用对应的释放函数），在 mimalloc v3 中，完成堆归属判断的时间复杂度为 `O(1)`，并不会引入过量开销，但注入前已创建的活跃内存将永远无法合入 mimalloc 堆，此外，每次释放时的条件分支判断也有可能影响 CPU 分支预测，进而对极致性能场景造成损耗。

`mimalloc-replace` 创造性地利用了 `realloc` 的语义特性，让重分配过程随着程序运行动态地将旧堆的活跃数据搬运至 mimalloc 堆，用一次移动 + 分支预测开销终结了双堆并存的遗留问题。

## 核心特性

**即插即用**：注入即完成接管，操作极简。

**智能指针归属判断**：利用 mimalloc 的 `mi_any_heap_contains` 函数（常数耗时）判断指针是否落在 mimalloc 堆内。若是，则交由 mimalloc 处理；若否，则触发迁移/释放逻辑。

**惰性数据迁移**：当对旧堆指针执行 `realloc`、`recalloc` 等重分配操作时，模块会：

1. 调用原 UCRT 的 `_msize` 获取旧内存大小
2. 在 mimalloc 堆中按照新尺寸申请内存
3. 将旧数据按照`realloc`/`recalloc`语义拷贝（`copy_nonoverlapping`）至新堆
4. 调用原 UCRT 的 `_free_base` 释放旧内存
5. 返回 mimalloc 的新指针

随着程序运行，旧堆的活跃数据会被逐渐“消化”并迁移至 mimalloc。

**防卸载保护**：在 `DLL_PROCESS_ATTACH` 时，模块会通过 `GetModuleHandleExW` 增加引用计数，防止注入的 DLL 在进程运行期间被意外卸载。

**全量 UCRT API 覆盖**：不仅覆盖基础的 `malloc`/`free`，还完整 Hook 了 `_aligned_*` 系列、`_expand`、`_msize`、`strdup`/`wcsdup`/`mbsdup` 等 UCRT 扩展函数。

**分配自举**：模块自身的内存分配直接使用自带的 mimalloc 分配器，不存在死锁问题。

**mimalloc 原始功能还原**：在启动时初始化全局进程堆，并在线程退出时正确调用 `mi_thread_done`，以保证线程资源的及时回收。

**自定义模块入口**: 使用 `entry` 编译参数指定模块入口，保证其尽快加载 Hook 完成内联。

## 覆盖的 API 列表

**基础分配**
`malloc`， `calloc`， `realloc`， `free`

**字符串/环境变量**
`_strdup`，`_wcsdup`， `_mbsdup`，`_dupenv_s`，`_wdupenv_s`

**堆信息/扩展**
`_expand`，`_msize`，`_recalloc`

**对齐分配**
`_aligned_malloc`， `_aligned_realloc`， `_aligned_recalloc`， `_aligned_msize`， `_aligned_free`

**带偏移对齐分配**
`_aligned_offset_malloc`，`_aligned_offset_realloc`， `_aligned_offset_recalloc`

## 使用

一般的 DLL 注入工具已经能够满足需求。

对于游戏场合，作者个人比较推荐使用 `Special K`，将本模块作为一个 Plugin 以 `Early` 优先级加载，这样能使它尽早完成注入接管内存。

## 局限性与注意事项

**非全量迁移**：只有被 `realloc` 等重分配函数触碰到的旧内存才会被迁移。如果某块旧内存自注入后只被读取，它将一直驻留在 UCRT 堆中，直到进程结束。

考虑到此时 UCRT 堆的全局锁已不再显著影响性能，此类惰性内存完全可以继续留在 UCRT 堆。

**个人发行版的变动**：作者个人的 Release 编译版本对 mimalloc 的编译参数做了如下修改：

1. 启用了平台 / 向量化支持，使用者需确认 CPU 支持 `haswell/avx2` 特性，时间范围大约在 2013 年以后。
2. 启用了 `MI_SKIP_COLLECT_ON_EXIT` 功能，在程序退出时不会主动回收内存，而是由操作系统自行完成。
    P.S. 这一条其实没有用，因为 `mimalloc-replace` 并没有调用 `mi_process_done`，本来就不会在退出时回收内存，这更像是声明(

如有需求请自行修改 `build.rs` 并编译。

## 关于初始化

个人发行版里,我启用了 `MI_WIN_INIT_USE_RAW_DLLMAIN`,这会让 mimalloc 进行两个操作:

1. 注册 `_pRawDllMain`，这是基于 CRT 的默认初始化回调，而 `entry` 指定了自定义入口，CRT 的默认初始化函数本来就不会运行。
2. 注册 TLS 回调，但在 DLL 模式下会被略过。

再考虑到 Rust/C 的链接符号处理，TLS 回调本身大概率不会留下来，哪怕留下来也会因为模块是 DLL 格式而不会工作，我们就可以安全地在自己的 dllmain 函数里安排 mimalloc 初始化 / 线程回收等操作了。

## DLC内容

此 repo 内还附带了一份简易 Zig 构建文件，可以利用它将 mimalloc 编译成可供 `mimalloc-redirect.dll` 使用的 DLL 链接库，同样默认启用了 `haswell/avx2` 特性、 `MI_SKIP_COLLECT_ON_EXIT` 和 `MI_WIN_INIT_USE_RAW_DLLMAIN`。

## 免责声明

该项目仅供实验、学习和交流使用。

请注意：

- hook 行为可能会在某些游戏中触发反作弊检测。
- UCRT 的内部机制和内存行为可能因 Windows 版本而异。
- 请负责任地使用 —— 风险自负。
