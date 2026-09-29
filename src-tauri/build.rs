fn main() {
    println!("cargo:rerun-if-changed=native/main.swift");
    println!("cargo:rerun-if-changed=Info.plist");
    println!("cargo:rerun-if-changed=native/Info.plist");
    println!("cargo:rerun-if-changed=parakeet/Package.swift");
    println!("cargo:rerun-if-changed=parakeet/Package.resolved");
    println!("cargo:rerun-if-changed=parakeet/Sources");
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::fs::create_dir_all(root.join("resources")).unwrap();
    let cache = root.join("target/swift-cache");
    std::fs::create_dir_all(&cache).unwrap();
    let status = std::process::Command::new("swiftc")
        .args([
            "-O",
            "-parse-as-library",
            "-target",
            "arm64-apple-macosx15.0",
            "-module-cache-path",
        ])
        .arg(cache)
        .arg(root.join("native/main.swift"))
        .args([
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__info_plist",
            "-Xlinker",
        ])
        .arg(root.join("native/Info.plist"))
        .arg("-o")
        .arg(root.join("resources/patter-native"))
        .status()
        .expect("Swift compiler is required for the native audio/calendar bridge");
    assert!(status.success(), "Native bridge compilation failed");
    let signed = std::process::Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--identifier",
            "gr.tau.patter.native",
        ])
        .arg(root.join("resources/patter-native"))
        .status()
        .expect("codesign is required for the native bridge");
    assert!(signed.success(), "Native bridge ad-hoc signing failed");
    let package = root.join("parakeet");
    let scratch = root.join("target/parakeet-build");
    let status = std::process::Command::new("swift")
        .args(["build", "--build-system", "native"])
        .arg("--package-path")
        .arg(&package)
        .arg("--scratch-path")
        .arg(&scratch)
        .args([
            "--configuration",
            "release",
            "--product",
            "PatterParakeet",
            "--force-resolved-versions",
        ])
        .status()
        .expect("Swift 6.2+ is required for Parakeet");
    assert!(status.success(), "Parakeet helper compilation failed");
    let binary = root.join("resources/patter-parakeet");
    std::fs::copy(scratch.join("release/PatterParakeet"), &binary).unwrap();
    let status = std::process::Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--identifier",
            "gr.tau.patter.parakeet",
        ])
        .arg(binary)
        .status()
        .unwrap();
    assert!(status.success(), "Parakeet helper signing failed");
    tauri_build::build();
}
