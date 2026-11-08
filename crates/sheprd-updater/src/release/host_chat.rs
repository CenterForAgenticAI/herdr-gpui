//! The `sheprd-acp` chats run on the machines that get a host build
//! (`host.rs`). Both targets' builds travel in one archive, with their own
//! signed manifest (schema 5) packed first, so a release stays under the
//! asset limit installed updaters read. The manifest's signature is
//! checked before any build in the archive is read, and the build against
//! the manifest before it leaves this computer.

use super::*;
use flate2::read::GzDecoder;

pub(super) const SCHEMA: u32 = 5;
/// The signed manifest and its signature, the archive's first two entries.
const MANIFEST: (&str, &str) = ("host-chat-manifest.json", "host-chat-manifest.sig");

/// The release asset holding every target's build.
pub(super) fn archive_name(version: &str) -> String {
    format!("sheprd-acp-{version}-linux-musl.tar.gz")
}

pub(super) fn accepts(version: &str, target: &str, asset: &str) -> bool {
    HOST_TARGETS.contains(&target) && asset == format!("sheprd-acp-{version}-{target}")
}

/// Reads the archive from `reader`, checks its manifest against `key_hex`
/// and `version`, and writes `target`'s build to `destination`, which must
/// not exist yet and is created private.
pub(super) fn unpack(
    reader: impl Read,
    version: &str,
    key_hex: &str,
    target: &str,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    if !HOST_TARGETS.contains(&target) {
        return Err(Error::UnsupportedTarget);
    }
    let mut archive = tar::Archive::new(GzDecoder::new(reader.take(ARCHIVE_LIMIT)));
    let mut entries = archive.entries().map_err(Error::ReadArchive)?;
    let mut next = |name: &str, limit: usize| -> Result<Vec<u8>> {
        let entry = entries
            .next()
            .ok_or(Error::HostChatLayout)?
            .map_err(Error::ReadArchive)?;
        if entry.path().map_err(Error::ReadArchive)? != Path::new(name) {
            return Err(Error::HostChatLayout);
        }
        read_bounded(entry, limit, cancel)
    };
    let manifest = next(MANIFEST.0, MANIFEST_LIMIT)?;
    let signature = next(MANIFEST.1, 64)?;
    let manifest = verify_manifest(&manifest, &signature, key_hex, version)?;
    // A manifest signed for another family must not answer for this one.
    if manifest.schema != SCHEMA {
        return Err(Error::ManifestSchema);
    }
    let asset = manifest
        .assets
        .into_iter()
        .find(|asset| asset.target == target && accepts(version, target, &asset.name))
        .ok_or(Error::MissingPlatformAsset)?;
    let wanted = Path::new(target).join("sheprd-acp");
    for entry in entries {
        let entry = entry.map_err(Error::ReadArchive)?;
        if entry.path().map_err(Error::ReadArchive)? != wanted {
            continue;
        }
        let mut file = create_archive(destination)?;
        copy_archive(entry, &mut file, &asset, cancel, |_, _| {})?;
        file.sync_all().map_err(Error::SyncArchive)?;
        return cancelled(cancel);
    }
    Err(Error::MissingPlatformAsset)
}

/// Downloads this release's `sheprd-acp` built for `target` into
/// `destination`, checked against the signed manifest in its archive by the
/// key this build embeds; a local build has neither and refuses. On error
/// the caller removes `destination`. Blocking: run it on a worker thread.
pub fn download_host_chat(target: &str, destination: &Path, cancel: &AtomicBool) -> Result<()> {
    let (version, key) =
        host::release_identity(crate::APP_VERSION, option_env!("HERDR_UPDATE_PUBLIC_KEY"))?;
    let response = request(
        &release_url(version, &archive_name(version)),
        RequestProfile::Archive,
        cancel,
    )?;
    unpack(
        response.into_body().into_reader(),
        version,
        key,
        target,
        destination,
        cancel,
    )
}
