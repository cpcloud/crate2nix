//! Utilities for calling `nix-prefetch` on packages.

use std::{convert::TryInto, collections::BTreeMap, sync::Arc};
use crate::{GenerateConfig, resolve::{CrateDerivation, ResolvedSource}};
use cargo_metadata::PackageId;
use failure::{bail, format_err, Error};
use futures::StreamExt;
use futures::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use serde::Deserialize;
use tokio::{fs, io::{self, AsyncWriteExt}, process::Command};

/// Uses `nix-prefetch` to get the hashes of the sources for the given packages if they come from
/// crates.io or git.
///
/// Uses and updates the existing hashes in the `config.crate_hash_json` file.
pub async fn prefetch(
    config: &GenerateConfig,
    crate_derivations: &mut [CrateDerivation],
) -> Result<BTreeMap<PackageId, String>, Error> {
    let hashes_string: String =
        std::fs::read_to_string(&config.crate_hashes_json).unwrap_or_else(|_| "{}".to_string());

    let old_hashes: BTreeMap<PackageId, String> = serde_json::from_str(&hashes_string)?;

    // Only copy used hashes over to the new map.
    let mut hashes: BTreeMap<PackageId, String> = BTreeMap::new();

    // Skip none-registry packages.
    let mut packages: Vec<&mut CrateDerivation> = crate_derivations
        .iter_mut()
        .filter(|c| match c.source {
            ResolvedSource::CratesIo { sha256: None, .. } => true,
            ResolvedSource::Git { .. } => true,
            _ => false,
        })
        .collect();
    let without_hash_num = packages
        .iter()
        .filter(|p| !old_hashes.contains_key(&p.package_id))
        .unique_by(|p| &p.source)
        .count();

    let progress_bar = Arc::new(ProgressBar::new(without_hash_num.try_into()?));
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .progress_chars("#>-"),
    );

    let old_hashes_ref = &old_hashes;
    let tasks = packages.iter().map(|package| {
        let pb = progress_bar.clone();
        async move {
            let sha256 = if let Some(hash) = old_hashes_ref.get(&package.package_id) {
                hash.trim().to_string()
            } else {
                let sha = match package.source {
                    ResolvedSource::CratesIo { .. } => nix_prefetch_from_crates_io(package).await?,
                    ResolvedSource::Git { .. } => nix_prefetch_from_git(package).await?,
                    _ => unreachable!(),
                };
                pb.inc(1);
                sha
            };
            Result::<_, Error>::Ok((
                package.source.with_sha256(sha256.clone()),
                package.package_id.clone(),
                sha256,
            ))
        }
    });

    // TODO: Is there a good way to choose this number?
    let n_concurrent_tasks = num_cpus::get() * 10;
    let triples: Vec<_> = futures::stream::iter(tasks)
        .buffer_unordered(n_concurrent_tasks)
        .try_collect().await?;

    for (package, (source, package_id, sha256)) in packages.iter_mut().zip(triples.into_iter()) {
        package.source = source;
        hashes.insert(package_id, sha256);
    }

    if hashes != old_hashes {
        fs::write(
            &config.crate_hashes_json,
            serde_json::to_vec_pretty(&hashes)?,
        ).await?;
        eprintln!(
            "Wrote hashes to {}.",
            config.crate_hashes_json.to_string_lossy()
        );
    }

    Ok(hashes)
}

async fn get_command_output(cmd: &str, args: &[&str]) -> Result<String, Error> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| format_err!("While spawning '{} {}': {}", cmd, args.join(" "), e))?;

    if !output.status.success() {
        io::stdout().write_all(&output.stdout).await?;
        io::stderr().write_all(&output.stderr).await?;
        bail!(
            "{}\n=> exited with: {}",
            cmd,
            output.status.code().unwrap_or(-1)
        );
    }

    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|_e| format_err!("output of '{} {}' is not UTF8!", cmd, args.join(" ")))
}

/// Invoke `nix-prefetch` for the given `package` and return the hash.
async fn nix_prefetch_from_crates_io(crate_derivation: &CrateDerivation) -> Result<String, Error> {
    let url = format!(
        "https://crates.io/api/v1/crates/{}/{}/download",
        crate_derivation.crate_name, crate_derivation.version
    );

    let cmd = "nix-prefetch-url";
    let args = [
        &url,
        "--name",
        &format!(
            "{}-{}",
            crate_derivation.crate_name, crate_derivation.version
        ),
    ];
    get_command_output(cmd, &args).await
}

/// A struct used to contain the output returned by `nix-prefetch-git`.
///
/// Additional fields are available (e.g., `name`), but we only call `nix-prefetch-git` to obtain
/// the nix sha256 for use in calls to `pkgs.fetchgit` in generated `Cargo.nix` files so there's no
/// reason to declare the fields here until they are needed.
#[derive(Deserialize)]
struct NixPrefetchGitInfo {
    sha256: String,
}

async fn nix_prefetch_from_git(crate_derivation: &CrateDerivation) -> Result<String, Error> {
    if let ResolvedSource::Git {
        url, rev, r#ref, ..
    } = &crate_derivation.source
    {
        let cmd = "nix-prefetch-git";
        let mut args = vec!["--url", url.as_str(), "--fetch-submodules", "--rev", rev];

        // TODO: --branch-name isn't documented in nix-prefetch-git --help
        // TODO: Consider the case when ref *isn't* a branch. You have to pass
        // that to `--rev` instead. This seems like a limitation of nix-prefetch-git.
        if let Some(r#ref) = r#ref {
            args.extend_from_slice(&["--branch-name", r#ref]);
        }

        let json = get_command_output(cmd, &args).await?;
        let prefetch_info: NixPrefetchGitInfo = serde_json::from_str(&json)?;
        Ok(prefetch_info.sha256)
    } else {
        Err(format_err!(
            "Invalid source type for pre-fetching using git"
        ))
    }
}
