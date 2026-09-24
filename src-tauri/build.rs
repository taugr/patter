fn main() {
    println!("cargo:rerun-if-changed=native/main.swift");
    println!("cargo:rerun-if-changed=Info.plist");
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
        .arg(root.join("Info.plist"))
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
    tauri_build::build();
}
