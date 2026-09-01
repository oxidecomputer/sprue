// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use dice_verifier::{Corim, ReferenceMeasurements, ReferenceMeasurementsError};
use futures::stream::{FuturesUnordered, StreamExt};
use regex::bytes::Regex;
use std::{
    io::{self, BufReader, Read, Seek, Write},
    path::Path,
};
use thiserror::Error;
use tracing::instrument;
use zip::ZipArchive;

use crate::config::SystemRelease;

#[derive(Debug, Error)]
pub enum MeasurementError {
    #[error("existing archive is corrupt")]
    ArchiveCorrput(io::Error),
    #[error("failed to download release")]
    Download(reqwest::Error),
    #[error("failed to extract artifact manifest")]
    MissingManifest,
    #[error("manifest references missing measurement")]
    MissingMeasurement,
    #[error("failed to open archive")]
    OpenArchive(zip::result::ZipError),
    #[error("failed to parse reference measurements")]
    ReferenceMeasurements(ReferenceMeasurementsError),
    #[error("failed to store release")]
    Storage(io::Error),
}

async fn download_release(
    release: &SystemRelease,
    release_path: &Path,
) -> Result<(), MeasurementError> {
    tracing::trace!(?release, "Downloading release repository");
    let mut response = reqwest::Client::new()
        .get(&release.url)
        .send()
        .await
        .map_err(MeasurementError::Download)?;

    let total_size = response.content_length();
    tracing::trace!(?total_size, "Computed release size");

    // Stream the (potentially multi-gigabyte) body straight to disk rather than
    // buffering it in memory. It is later read back from disk.
    let mut file = std::fs::File::create(release_path).map_err(MeasurementError::Storage)?;
    let mut downloaded = 0u64;
    // Tracks the next 10% milestone (10, 20, ..., 100) we still need to log.
    let mut next_milestone = 10u64;

    while let Some(chunk) = response.chunk().await.map_err(MeasurementError::Download)? {
        file.write_all(&chunk).map_err(MeasurementError::Storage)?;
        downloaded += chunk.len() as u64;

        if let Some(total_size) = total_size.filter(|size| *size > 0) {
            let percent = downloaded * 100 / total_size;
            while percent >= next_milestone && next_milestone <= 100 {
                tracing::trace!(
                    percent = next_milestone,
                    downloaded,
                    total = total_size,
                    "Downloading release repository"
                );
                next_milestone += 10;
            }
        }
    }

    file.flush().map_err(MeasurementError::Storage)?;
    Ok(())
}

fn extract_corim<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<Corim>, MeasurementError> {
    let manifest_pattern =
        Regex::new("repo/targets/[0-9a-f]*.artifacts.json").expect("Known valid regex");
    let manifest_name = archive
        .file_names()
        .find(|name| manifest_pattern.is_match(name.as_bytes()))
        .map(|name| name.to_owned())
        .ok_or(MeasurementError::MissingManifest)?;
    tracing::trace!(?manifest_name, "Extracting manifest name");
    let manifest = {
        let mut data = Vec::new();
        archive
            .by_name(&manifest_name)
            .expect("Known valid file name")
            .read_to_end(&mut data)
            .unwrap();
        String::from_utf8_lossy(&data).into_owned()
    };

    let corpus_pattern = Regex::new("measurement_corpus-(?:staging|production)-corim.*?.tar.gz")
        .expect("Known valid regex");
    let corpus_files = corpus_pattern
        .find_iter(manifest.as_bytes())
        .map(|m| String::from_utf8_lossy(m.as_bytes()))
        .collect::<Vec<_>>();
    tracing::trace!(?corpus_files, "Extracting corpus file locations");

    let measurements = corpus_files
        .iter()
        .map(|name| {
            let mut data = Vec::new();
            let full_file_name = {
                let full_file_name = archive
                    .file_names()
                    .find(|f| f.ends_with(name.as_ref()))
                    .ok_or(MeasurementError::MissingMeasurement)?;
                full_file_name.to_string()
            };
            archive
                .by_name(&full_file_name)
                .expect("Known valid file name")
                .read_to_end(&mut data)
                .expect("Read data from known file");
            Ok(Corim::from_bytes(&data).unwrap())
        })
        .collect::<Result<Vec<_>, MeasurementError>>()?;

    Ok(measurements)
}

/// Computes the SHA-256 of the archive at `path` by streaming it through a
/// fixed-size buffer, so memory stays bounded regardless of archive size.
///
/// Hashing several gigabytes is CPU- and I/O-bound, so it runs on a blocking
/// thread to avoid stalling the async runtime.
async fn checksum_release(path: &Path) -> Result<String, MeasurementError> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        let mut reader =
            BufReader::new(std::fs::File::open(&path).map_err(MeasurementError::ArchiveCorrput)?);
        let mut hasher = Sha256::new();
        io::copy(&mut reader, &mut hasher).map_err(MeasurementError::ArchiveCorrput)?;
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await
    .expect("checksum task should not panic")
}

#[instrument(skip_all, fields(version = release.version, checksum = release.checksum))]
async fn fetch_release(
    release: &SystemRelease,
    release_path: &Path,
) -> Result<std::fs::File, MeasurementError> {
    let valid = if std::fs::File::open(release_path).is_ok() {
        tracing::trace!("Found existing repository");
        tracing::info!("Testing checksum on repository");

        let checksum = checksum_release(release_path).await?;
        if checksum != release.checksum {
            tracing::warn!(
                "Release checksum mismatch: expected {}, got {}. Redownload will be triggered.",
                release.checksum,
                checksum
            );
            false
        } else {
            tracing::info!("Release checksum is valid");
            true
        }
    } else {
        tracing::info!(
            ?release,
            "Repository file for release not found. Download will be triggered."
        );
        false
    };

    if !valid {
        download_release(release, release_path).await?;
    }

    std::fs::File::open(release_path).map_err(MeasurementError::Storage)
}

async fn parse_release(
    release: &SystemRelease,
    release_path: &Path,
) -> Result<Vec<Corim>, MeasurementError> {
    let file = fetch_release(release, release_path).await?;
    let mut archive =
        ZipArchive::new(BufReader::new(file)).map_err(MeasurementError::OpenArchive)?;
    let measurements = extract_corim(&mut archive)?;
    Ok(measurements)
}

/// Maximum number of releases to fetch and parse concurrently.
const MAX_CONCURRENT_RELEASES: usize = 3;

async fn fetch_and_parse(
    release: &SystemRelease,
    release_storage: &Path,
) -> Result<Vec<Corim>, MeasurementError> {
    let release_path = release_storage.join(&release.version);
    parse_release(release, &release_path).await
}

pub async fn fetch_measurements(
    releases: &[SystemRelease],
    release_storage: &Path,
) -> Result<ReferenceMeasurements, MeasurementError> {
    let mut remaining = releases.iter();
    let mut in_flight = FuturesUnordered::new();

    // Prime the pool up to the concurrency limit.
    for release in remaining.by_ref().take(MAX_CONCURRENT_RELEASES) {
        in_flight.push(fetch_and_parse(release, release_storage));
    }

    let mut measurements = Vec::new();
    // As each release finishes, record its measurements and start the next one
    // so that up to MAX_CONCURRENT_RELEASES are always in progress.
    while let Some(result) = in_flight.next().await {
        measurements.extend(result?);
        if let Some(release) = remaining.next() {
            in_flight.push(fetch_and_parse(release, release_storage));
        }
    }

    tracing::trace!(
        count = measurements.len(),
        "Retrieved reference measurements"
    );
    Ok(TryFrom::<&[Corim]>::try_from(&measurements)
        .map_err(MeasurementError::ReferenceMeasurements)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rats_corim::CorimBuilder;
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    const MANIFEST: &str = "repo/targets/0123456789abcdef.artifacts.json";
    const STAGING: &str = "measurement_corpus-staging-corim-all-sp-v1.0.67-1.0.67.tar.gz";
    const PRODUCTION: &str =
        "measurement_corpus-production-corim-oxide-rot-1-v1.0.38-1.0.38.tar.gz";

    fn corim(id: &str) -> Vec<u8> {
        let mut builder = CorimBuilder::new();
        builder.id(id.to_string());
        builder.vendor("test-vendor".to_string());
        builder.tag_id("test-tag".to_string());
        builder.version("1.0.0".to_string());
        builder.add_hash("test-measurement".to_string(), 1, vec![0xab; 32]);
        builder
            .build()
            .expect("Valid corim")
            .to_vec()
            .expect("Serializable corim")
    }

    /// Builds a manifest listing `targets`, in the shape the TUF repository
    /// uses: each entry references a target by its hash-prefixed file name.
    fn manifest(targets: &[&str]) -> Vec<u8> {
        let artifacts = targets
            .iter()
            .map(|target| format!(r#"{{"name":"corpus","target":"{}"}}"#, target_name(target)))
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"artifacts":[{artifacts}]}}"#).into_bytes()
    }

    fn target_name(name: &str) -> String {
        format!("{}.{name}", "0".repeat(64))
    }

    fn archive(entries: Vec<(String, Vec<u8>)>) -> ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(name, SimpleFileOptions::default())
                .expect("Started archive entry");
            writer.write_all(&data).expect("Wrote archive entry");
        }
        ZipArchive::new(writer.finish().expect("Finished archive")).expect("Opened archive")
    }

    /// Builds an archive containing a manifest listing `targets` plus a corim
    /// for each of them, mirroring the layout of a real release repository.
    fn release(targets: &[&str]) -> ZipArchive<Cursor<Vec<u8>>> {
        let mut entries = vec![(MANIFEST.to_string(), manifest(targets))];
        entries.extend(targets.iter().map(|target| {
            (
                format!("repo/targets/{}", target_name(target)),
                corim(target),
            )
        }));
        archive(entries)
    }

    #[test]
    fn extracts_every_corpus_in_the_manifest() {
        let measurements =
            extract_corim(&mut release(&[STAGING, PRODUCTION])).expect("Extracted measurements");

        assert_eq!(
            measurements
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec![STAGING, PRODUCTION]
        );
    }

    #[test]
    fn skips_artifacts_that_are_not_a_corpus() {
        let measurements = extract_corim(&mut release(&[
            "host-1.0.67.tar.gz",
            STAGING,
            "trampoline-1.0.67.tar.gz",
        ]))
        .expect("Extracted measurements");

        assert_eq!(
            measurements
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec![STAGING]
        );
    }

    #[test]
    fn skips_corpora_from_unknown_channels() {
        let measurements = extract_corim(&mut release(&[
            "measurement_corpus-dev-corim-all-sp-v1.0.67.tar.gz",
        ]))
        .expect("Extracted measurements");

        assert!(measurements.is_empty());
    }

    #[test]
    fn errors_when_the_manifest_is_absent() {
        let mut archive = archive(vec![(
            format!("repo/targets/{}", target_name(STAGING)),
            corim(STAGING),
        )]);

        assert!(matches!(
            extract_corim(&mut archive),
            Err(MeasurementError::MissingManifest)
        ));
    }

    #[test]
    fn errors_when_a_referenced_corpus_is_absent() {
        let mut archive = archive(vec![(MANIFEST.to_string(), manifest(&[STAGING]))]);

        assert!(matches!(
            extract_corim(&mut archive),
            Err(MeasurementError::MissingMeasurement)
        ));
    }
}
