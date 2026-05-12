const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // const mi_secure = b.option(bool, "secure", "Use security mitigations") orelse false;
    // const mi_secure_full = b.option(bool, "secure-full", "Use full security mitigations (implies secure)") orelse false;
    // const mi_override = b.option(bool, "override", "Override standard malloc interface") orelse true;
    // const mi_debug_full = b.option(bool, "debug-full", "Enable full internal heap invariant checking") orelse false;
    // const mi_build_shared = b.option(bool, "shared", "Build shared library (DLL/.so)") orelse true;
    // const mi_build_static = b.option(bool, "static", "Build static library (.lib/.a)") orelse true;
    // const mi_build_tests = b.option(bool, "tests", "Build test executables") orelse false;
    // const mi_win_redirect = b.option(bool, "win-redirect", "Use redirection module on Windows") orelse true;
    // const mi_opt_simd = b.option(bool, "opt-simd", "Use SIMD instructions (requires MI_OPT_ARCH to be enabled)") orelse false;
    // const mi_raw_dllmain = b.option(bool, "raw-dllmain", "Use the raw DLL main entry point for mimalloc initialization; can be more robust but can also lead to link errors with other libraries") orelse false;
    // const mi_skip_collect = b.option(bool, "skip-collect", "Skip collecting memory on program exit") orelse false;

    const shared_sources = [_][]const u8{
        "src/alloc.c",
        "src/alloc-aligned.c",
        "src/alloc-posix.c",
        "src/arena.c",
        "src/arena-meta.c",
        "src/bitmap.c",
        "src/heap.c",
        "src/init.c",
        "src/libc.c",
        "src/options.c",
        "src/os.c",
        "src/page.c",
        "src/page-map.c",
        "src/random.c",
        "src/stats.c",
        "src/theap.c",
        "src/threadlocal.c",
        "src/prim/prim.c",
    };

    const shared_flags = [_][]const u8{
        "-march=haswell",
        "-mavx2",
        "-flto=full",
        "-Wno-date-time",
        "-fms-extensions",
        "-O3",
        "-fvisibility=hidden",
        "-ftls-model=initial-exec",
    };

    const shared_x86_flags = [_][]const u8{
        "-flto=full",
        "-Wno-date-time",
        "-fms-extensions",
        "-O3",
        "-fvisibility=hidden",
        "-ftls-model=initial-exec",
    };

    var mimodule = b.createModule(.{ .target = target, .optimize = optimize, .link_libc = true, .strip = true });
    mimodule.addCSourceFiles(.{ .root = b.path("mimalloc"), .files = &shared_sources, .flags = if (target.result.cpu.arch == .x86) &shared_x86_flags else &shared_flags });
    mimodule.addIncludePath(b.path("mimalloc/include"));
    mimodule.addCMacro("MI_WIN_INIT_USE_RAW_DLLMAIN", "1");
    mimodule.addCMacro("MI_SKIP_COLLECT_ON_EXIT", "1");
    mimodule.addCMacro("MI_MALLOC_OVERRIDE", "");
    mimodule.addCMacro("MI_SHARED_LIB_EXPORT", "");
    mimodule.addCMacro("MI_SHARED_LIB", "");
    if (target.result.cpu.arch == .x86) {
        mimodule.addObjectFile(b.path("mimalloc/bin/mimalloc-redirect32.lib"));
    } else {
        mimodule.addCMacro("MI_OPT_SIMD", "1");
        mimodule.addObjectFile(b.path("mimalloc/bin/mimalloc-redirect.lib"));
    }
    // mimodule.linkSystemLibrary("psapi", .{});
    // mimodule.linkSystemLibrary("shell32", .{});
    // mimodule.linkSystemLibrary("user32", .{});
    // mimodule.linkSystemLibrary("advapi32", .{});
    // mimodule.linkSystemLibrary("bcrypt", .{});

    const libmimalloc = b.addLibrary(.{
        .name = "mimalloc",
        .linkage = .dynamic,
        .version = .{ .major = 3, .minor = 3, .patch = 3 },
        .root_module = mimodule,
    });

    b.installArtifact(libmimalloc);
}

// zig build -Doptimize=ReleaseFast -Dtarget=x86-windows-gnu
// zig build -Doptimize=ReleaseFast -Dtarget=x86_64-windows-gnu
