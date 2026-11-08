//! The host builds' `sheprd-acp`: one archive per release, its signed
//! manifest (schema 5) first, read only through that manifest.

use super::*;

const VERSION: &str = "20261108.3";
const TARGET: &str = "aarch64-unknown-linux-musl";

fn key() -> String {
    hex(SigningKey::from_bytes(&[42; 32]).verifying_key().as_bytes())
}

fn manifest(build: &[u8]) -> Manifest {
    Manifest {
        schema: host_chat::SCHEMA,
        version: VERSION.into(),
        assets: HOST_TARGETS
            .iter()
            .map(|target| Asset {
                target: (*target).into(),
                name: format!("sheprd-acp-{VERSION}-{target}"),
                size: build.len() as u64,
                sha256: hex(&Sha256::digest(build)),
            })
            .collect(),
    }
}

/// The archive the release packs, its manifest signed by `signer` and
/// `build` as every target's program.
fn archive(manifest: &Manifest, signer: u8, build: &[u8]) -> Vec<u8> {
    let bytes = serde_json::to_vec(manifest).unwrap_or_default();
    let signature = SigningKey::from_bytes(&[signer; 32])
        .sign(&bytes)
        .to_bytes();
    let mut packed = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut add = |name: &str, data: &[u8]| {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        packed.append_data(&mut header, name, data).unwrap_or(());
    };
    add("host-chat-manifest.json", &bytes);
    add("host-chat-manifest.sig", &signature);
    for target in HOST_TARGETS {
        add(&format!("{target}/sheprd-acp"), build);
    }
    packed
        .into_inner()
        .and_then(|gz| gz.finish())
        .unwrap_or_default()
}

fn unpack(archive: &[u8], target: &str) -> (Result<()>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
    let result = host_chat::unpack(
        archive,
        VERSION,
        &key(),
        target,
        &dir.path().join("sheprd-acp"),
        &AtomicBool::new(false),
    );
    (result, dir)
}

#[test]
fn a_build_signed_for_this_release_is_unpacked() -> anyhow::Result<()> {
    let build = b"static sheprd-acp";
    let (result, dir) = unpack(&archive(&manifest(build), 42, build), TARGET);
    result?;
    assert_eq!(std::fs::read(dir.path().join("sheprd-acp"))?, build);
    Ok(())
}

#[test]
fn nothing_is_written_without_the_right_signature_schema_or_target() {
    let build = b"static sheprd-acp";
    let (result, dir) = unpack(&archive(&manifest(build), 7, build), TARGET);
    assert!(matches!(result, Err(Error::Signature(_))));
    assert!(!dir.path().join("sheprd-acp").exists());
    // The host build manifest, signed by the same key, answers for no chat.
    let mut hosts = manifest(build);
    hosts.schema = 4;
    let (result, _) = unpack(&archive(&hosts, 42, build), TARGET);
    assert!(result.is_err());
    let (result, _) = unpack(
        &archive(&manifest(build), 42, build),
        "x86_64-unknown-linux-gnu",
    );
    assert!(matches!(result, Err(Error::UnsupportedTarget)));
}

#[test]
fn a_build_that_differs_from_its_manifest_is_refused() {
    let (result, _) = unpack(
        &archive(&manifest(b"signed bytes"), 42, b"other bytes!"),
        TARGET,
    );
    assert!(matches!(result, Err(Error::ArchiveDigest)));
}

#[test]
fn an_archive_without_its_manifest_first_is_refused() {
    let mut packed = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(3);
    header.set_cksum();
    let _ = packed.append_data(&mut header, format!("{TARGET}/sheprd-acp"), &b"abc"[..]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = Write::write_all(&mut gz, &packed.into_inner().unwrap_or_default());
    let (result, _) = unpack(&gz.finish().unwrap_or_default(), TARGET);
    assert!(matches!(result, Err(Error::HostChatLayout)));
}

#[test]
fn the_archive_is_named_for_the_release() {
    assert_eq!(
        host_chat::archive_name(VERSION),
        "sheprd-acp-20261108.3-linux-musl.tar.gz"
    );
}
