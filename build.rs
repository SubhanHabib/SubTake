fn main() {
    if std::env::var_os("CARGO_FEATURE_NATIVE_FFMPEG").is_some() {
        println!("cargo:rerun-if-changed=scripts/decoder.c");
        println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
        let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let root = std::env::var_os("FFMPEG_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(if target == "macos" {
                    if arch == "aarch64" {
                        "/opt/homebrew"
                    } else {
                        "/usr/local"
                    }
                } else {
                    "/usr"
                })
            });
        let mut build = cc::Build::new();
        build
            .file("scripts/decoder.c")
            .std("c11")
            .include(root.join("include"));
        if target == "linux" {
            build.include(root.join(format!(
                "include/{}-linux-gnu",
                if arch == "aarch64" {
                    "aarch64"
                } else {
                    "x86_64"
                }
            )));
        }
        build
            .flag_if_supported("-Wno-deprecated-declarations")
            .compile("subtake_decoder");
        println!(
            "cargo:rustc-link-search=native={}",
            root.join("lib").display()
        );
        for library in ["avformat", "avcodec", "avutil", "swscale"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-headerpad_max_install_names");
        println!("cargo:rerun-if-changed=scripts/file-events.m");
        println!("cargo:rerun-if-changed=scripts/recorder-glass.m");
        println!("cargo:rerun-if-changed=scripts/brand-mark.h");
        cc::Build::new()
            .file("scripts/file-events.m")
            .file("scripts/recorder-glass.m")
            .flag("-fobjc-arc")
            .flag("-mmacosx-version-min=14.0")
            .compile("subtake_file_events");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Carbon");
    }
    slint_build::compile("ui/editor.slint").expect("compile native editor interface");
}
