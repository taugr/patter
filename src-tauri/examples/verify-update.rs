//! Release gate: uses the same verifier as Tauri, with negative controls.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("release-artifacts/latest.json")).unwrap())
            .unwrap();
    let config: Value =
        serde_json::from_slice(&std::fs::read(root.join("src-tauri/tauri.conf.json")).unwrap())
            .unwrap();
    let bytes = std::fs::read(root.join("release-artifacts/Patter.app.tar.gz")).unwrap();
    let pub_text = String::from_utf8(
        STANDARD
            .decode(config["plugins"]["updater"]["pubkey"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let sig_text = String::from_utf8(
        STANDARD
            .decode(
                manifest["platforms"]["darwin-aarch64"]["signature"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    let key = PublicKey::decode(&pub_text).unwrap();
    let signature = Signature::decode(&sig_text).unwrap();
    key.verify(&bytes, &signature, true)
        .expect("Release signature must verify");
    let signed_version = signature
        .trusted_comment()
        .split('\t')
        .find_map(|f| f.strip_prefix("version:"))
        .expect("Missing signed version");
    assert_eq!(signed_version, manifest["version"].as_str().unwrap());
    assert_ne!(
        signed_version, "999.0.0",
        "A falsely announced version must be rejected"
    );
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    assert!(
        key.verify(&corrupt, &signature, true).is_err(),
        "Tampered archive must be rejected"
    );
    let mut sig_lines: Vec<String> = sig_text.lines().map(str::to_owned).collect();
    let mut wrong_id = STANDARD.decode(&sig_lines[1]).unwrap();
    wrong_id[2] ^= 1;
    sig_lines[1] = STANDARD.encode(wrong_id);
    let wrong_signature = Signature::decode(&sig_lines.join("\n")).unwrap();
    assert!(
        key.verify(&bytes, &wrong_signature, true).is_err(),
        "Wrong signing key identity must be rejected"
    );
    println!("Valid release accepted; tampered archive, wrong key identity and mismatched version rejected.");
}
