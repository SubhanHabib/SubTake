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
        build_native_swift();
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=WebKit");
        println!("cargo:rustc-link-lib=framework=Carbon");
    }
}

/// Compiles `native/*.swift` into one static library. The Swift files export
/// plain C symbols with `@_cdecl`, which Rust declares in `extern "C"` blocks.
/// The Swift runtime ships with macOS 14, so nothing is bundled with the app.
fn build_native_swift() {
    use std::{path::PathBuf, process::Command};

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64".to_owned(),
        Ok(other) => other.to_owned(),
        Err(_) => panic!("cargo sets CARGO_CFG_TARGET_ARCH"),
    };

    println!("cargo:rerun-if-changed=native");
    let mut sources: Vec<PathBuf> = std::fs::read_dir("native")
        .expect("native/ holds the macOS Swift sources")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "swift")
        })
        .collect();
    sources.sort();
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }

    let library = out.join("libsubtake_macos.a");
    let output = Command::new("xcrun")
        .args(["swiftc", "-emit-library", "-static", "-parse-as-library"])
        .args(["-module-name", "SubTakeMacOS", "-swift-version", "5"])
        // A debug build keeps Swift's trap messages, which name the file and
        // line; an optimised one reduces every failed check to a bare trap.
        .arg(if std::env::var("PROFILE").as_deref() == Ok("release") {
            "-O"
        } else {
            "-Onone"
        })
        .args(["-target", &format!("{arch}-apple-macos14.0")])
        .arg("-o")
        .arg(&library)
        .args(&sources)
        .output()
        .expect("xcrun swiftc runs");
    assert!(
        output.status.success(),
        "swiftc failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=subtake_macos");

    // The Swift objects autolink swiftCore and the overlays; these are where
    // the linker finds them. The SDK has the runtime stubs, the toolchain the
    // back-deployment shims.
    let sdk = command_line(&["--sdk", "macosx", "--show-sdk-path"]);
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    let swiftc = PathBuf::from(command_line(&["--find", "swiftc"]));
    if let Some(toolchain) = swiftc.parent().and_then(|bin| bin.parent()) {
        let shims = toolchain.join("lib/swift/macosx");
        println!("cargo:rustc-link-search=native={}", shims.display());
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}

fn command_line(arguments: &[&str]) -> String {
    let output = std::process::Command::new("xcrun")
        .args(arguments)
        .output()
        .expect("xcrun runs");
    assert!(output.status.success(), "xcrun {arguments:?} failed");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
