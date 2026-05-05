use std::env;
use std::path::Path;

fn main() {
    let mut build = cc::Build::new();
    let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest = Path::new(&cargo_manifest_dir);
    // let include_dir = Path::new(&cargo_manifest_dir)
    //     .join("mimalloc/")
    //     .join("include")
    //     .to_str()
    //     .expect("include path is not valid UTF-8")
    //     .to_string();
    let inc = manifest.join("mimalloc").join("include");
    let src = manifest.join("mimalloc").join("src");

    // println!("cargo:INCLUDE_DIR={include_dir}");
    build.include(inc);
    build.include(&src);
    build.file(src.join("static.c"));

    build.define("MI_OVERRIDE", "OFF");
    build.define("MI_WIN_REDIRECT", "OFF");
    build.define("MI_BUILD_OBJECT", "OFF");
    build.define("MI_BUILD_SHARED", "OFF");
    build.define("MI_BUILD_TESTS", "OFF");
    build.define("MI_OPT_ARCH", "ON");
    build.define("MI_OPT_SIMD", "ON");
    build.define("MI_XMALLOC", "ON");
    build.define("MI_USE_CXX", "ON");
    if env::var_os("CARGO_FEATURE_DEBUG").is_some() {
        build.define("MI_DEBUG", "3");
        build.define("MI_SHOW_ERRORS", "1");
    } else {
        // Remove heavy debug assertions etc
        build.define("MI_DEBUG", "0");
    }
    if build.get_compiler().is_like_msvc() || build.get_compiler().is_like_clang_cl() {
        build.cpp(true).flag("/O2").flag("/Ob2").flag("/Z7");
    } else {
        build
            .flag("-Wno-unknown-pragmas")
            .flag("-flto=thin")
            .flag("-O3")
            .flag("-fomit-frame-pointer")
            .flag("-funroll-loops");
    }
    build.compile("mimalloc");
    // Link with libs needed on Windows
    // https://github.com/microsoft/mimalloc/blob/af21001f7a65eafb8fb16460b018ebf9d75e2ad8/CMakeLists.txt#L487
    for lib in ["psapi", "shell32", "user32", "advapi32", "bcrypt"] {
        println!("cargo:rustc-link-lib={}", lib);
    }
    println!("cargo:rustc-link-arg=-Wl,--entry=raw_main");
}
