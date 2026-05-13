# Mimalloc Replace

English | [中文](./readme-zh_CN.md)

`mimalloc-replace` is an injection module that dynamically hooks the default allocator of the Windows UCRT, forwarding corresponding calls to the built-in mimalloc allocator and using reallocation functions to migrate active data from the UCRT heap into the mimalloc heap.

## Motivation

mimalloc itself provides `mimalloc-redirect.dll` and `minject.exe`, which can replace allocation functions at startup by modifying the program’s IAT. Generally this approach is efficient enough, but for programs like games that update frequently and demand extreme ease of injection, IAT modification becomes less flexible.

While DLL injection combined with inline hooking can conveniently take over the allocator, it introduces the problem of how to deal with memory already allocated before the hook was installed.

If you simply route `free` or `realloc` to mimalloc, mimalloc will try to release memory allocated by the UCRT, causing a crash.

A common solution is to maintain dual heaps – determine which heap a pointer belongs to and then call the corresponding release function. In mimalloc v3, heap ownership checks run in `O(1)` time and do not introduce excessive overhead. However, active memory that existed before the injection will never be merged into the mimalloc heap. Moreover, the conditional branch on every deallocation can affect CPU branch prediction, potentially hurting performance in extreme scenarios.

`mimalloc-replace` creatively exploits the semantics of `realloc` so that the reallocation process dynamically moves active data from the old heap to the mimalloc heap as the program runs. With a single move plus a branch-prediction cost, it eliminates the lingering problem of dual-heap coexistence.

## Core Features

**Plug and play** – Injection immediately completes the takeover. Operation is extremely simple.

**Smart pointer ownership detection** – Uses mimalloc’s `mi_any_heap_contains` function (constant-time) to check whether a pointer lives inside a mimalloc heap. If yes, mimalloc handles it; if not, the migration / free logic is triggered.

**Lazy data migration** – When a reallocation operation (`realloc`, `recalloc`, etc.) is called on an old-heap pointer, the module will:

1. Call the original UCRT `_msize` to obtain the old memory size.
2. Allocate a new block of the requested size from the mimalloc heap.
3. Copy the old data to the new heap according to the semantics of `realloc`/`recalloc` (using `copy_nonoverlapping`).
4. Call the corresponding original UCRT free function to free the old memory.
5. Return the new mimalloc pointer.

As the program runs, active data from the old heap is gradually “digested” and migrated into mimalloc.

**Anti-unload protection** – At `DLL_PROCESS_ATTACH`, the module increments its own reference count via `GetModuleHandleExW`, preventing the injected DLL from being accidentally unloaded while the process is running.

**Full UCRT API coverage** – Not only basic `malloc`/`free`, but also complete hooks for `_aligned_*` series, `_expand`, `_msize`, `strdup`/`wcsdup`/`mbsdup`, and other UCRT extension functions.

**Self-hosting allocation** – The module’s own memory allocations directly use the built-in mimalloc allocator, so no deadlocks can occur.

**Restoring original mimalloc behaviour** – Initializes the global process heap at startup and correctly calls `mi_thread_done` on thread exit to ensure timely reclamation of thread resources.

**Custom module entry point** – Uses the `entry` linker option to specify a custom entry point, ensuring hooks are installed as early as possible.

## Covered API List

**Basic allocation**:
`malloc`, `calloc`, `realloc`, `free`

**Strings / environment**:
`_strdup`, `_wcsdup`, `_mbsdup`, `_dupenv_s`, `_wdupenv_s`

**Heap information / expansion**:
`_expand`, `_msize`, `_recalloc`

**Aligned allocation**:
`_aligned_malloc`, `_aligned_realloc`, `_aligned_recalloc`, `_aligned_msize`, `_aligned_free`

**Aligned allocation with offset**:
`_aligned_offset_malloc`, `_aligned_offset_realloc`, `_aligned_offset_recalloc`

## Usage

Generic DLL injection tools are sufficient.

For games, the author personally recommends using **Special K**, loading this module as a Plugin with **Early** priority so that it injects and takes over memory as early as possible.

## Limitations and Notes

**Non-total migration** – Only old memory that is touched by reallocation functions (like `realloc`) is migrated. If a block of old memory is only ever read after injection, it will remain on the UCRT heap until the process exits.

Given that the UCRT heap’s global lock no longer meaningfully affects performance at that point, such lazy memory can perfectly well stay on the UCRT heap.

**Personal build modifications** – The author's release binaries are compiled with the following changes to mimalloc:

1. Platform / vectorization support is enabled. Users need to ensure their CPU supports **Haswell / AVX2** features (roughly 2013 and later).
2. `MI_SKIP_COLLECT_ON_EXIT` is enabled, so memory is not actively reclaimed on process exit; the OS handles it instead.  
   P.S. This is actually useless here because `mimalloc-replace` never calls `mi_process_done`, so there is no exit-time reclamation anyway. Think of it more as a declaration of intent.

If you have different requirements, please modify `build.rs` and rebuild.

## About Initialisation

In the author's personal build, `MI_WIN_INIT_USE_RAW_DLLMAIN` is enabled. This makes mimalloc attempt two things:

1. Register a `_pRawDllMain` based on the CRT’s default initialisation callback. However, because `entry` specifies a custom entry point, the CRT’s default initialisation function will not run anyway.
2. Register a TLS callback, which is skipped in DLL mode.

Furthermore, considering how Rust/C symbol linkage works, the TLS callback most likely won’t even remain; even if it did, it would not function because the module is a DLL. Therefore we can safely arrange mimalloc initialisation and thread teardown operations inside our own dllmain function.

## DLC Content

This repository also contains a simple Zig build file that can be used to compile mimalloc into a DLL import library suitable for use with `mimalloc-redirect.dll`. It also enables Haswell/AVX2 features, `MI_SKIP_COLLECT_ON_EXIT`, and `MI_WIN_INIT_USE_RAW_DLLMAIN` by default.

## Disclaimer

This project is for experimentation, learning and exchange purposes only.

Please note:

- Hooking behaviour may trigger anti-cheat detection in some games.
- UCRT internal mechanisms and memory behaviour may vary between Windows versions.
- Use responsibly – at your own risk.
