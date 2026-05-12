use std::env;
use std::path::Path;

fn main() {
    let mut build = cc::Build::new();
    let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest = Path::new(&cargo_manifest_dir).join("mimalloc");

    build
        .file(manifest.join("src").join("static.c"))
        .include(manifest.join("include"));

    build
        .define("MI_WIN_INIT_USE_RAW_DLLMAIN", "1")
        .define("MI_SKIP_COLLECT_ON_EXIT", "1")
        .define("MI_MALLOC_OVERRIDE", None)
        .define("MI_STATIC_LIB", None);

    if env::var_os("CARGO_FEATURE_DEBUG").is_some() {
        build.define("MI_DEBUG", "3").define("MI_SHOW_ERRORS", "1");
    } else {
        build.define("MI_DEBUG", "0");
    }
    if build.get_compiler().is_like_msvc() || build.get_compiler().is_like_clang_cl() {
        if cfg!(target_feature = "avx2") {
            build.flag("/arch:AVX2").define("MI_OPT_SIMD", "1");
        }
        build
            .cpp(true)
            .flag("/O2")
            .flag("/Ob2")
            .flag("/Oi")
            .flag("/Oy")
            .flag("/Gy")
            .flag("/GL")
            .flag("/GT")
            .flag("/Zc:inline"); // idk if this works
        println!("cargo:rustc-link-arg=/ENTRY:raw_main");
    } else {
        if cfg!(target_feature = "avx2") {
            build
                .flag("-march=haswell")
                .flag("-mavx2")
                .define("MI_OPT_SIMD", "1");
        }
        build
            .flag("-Wno-date-time")
            .flag("-flto=thin")
            .flag("-O3")
            .flag("-fms-extensions")
            .flag("-fvisibility=hidden")
            .flag("-ftls-model=initial-exec");
        println!("cargo:rustc-link-arg=-Wl,--entry=raw_main");
    }
    // link with libs needed on Windows according to cmakelist
    for lib in ["psapi", "shell32", "user32", "advapi32", "bcrypt"] {
        println!("cargo:rustc-link-lib={}", lib);
    }
    build.compile("mimalloc");
}
