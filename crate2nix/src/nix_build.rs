//! Code for invoking nix_build.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use failure::bail;
use failure::format_err;
use failure::Error;

/// Call `nix build` in the given directory on the `default.nix` in that directory.
pub async fn nix_build(
    project_dir: impl AsRef<Path>,
    nix_attr: &str,
    features: &[&str],
) -> Result<(), Error> {
    let project_dir = project_dir.as_ref();
    let project_dir_display = project_dir.display();

    eprintln!("Building {}.", project_dir_display);
    let status = Command::new("nix")
        .current_dir(&project_dir)
        .args(&[
            "--show-trace",
            "build",
            "-f",
            "default.nix",
            nix_attr,
            "--arg",
            "rootFeatures",
        ])
        .arg(format!(
            "[ {} ]",
            features
                .iter()
                .map(|s| crate::render::escape_nix_string(s))
                .collect::<Vec<_>>()
                .join(" ")
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status().await
        .map_err(|e| format_err!("while spawning nix-build for {}: {}", project_dir_display, e))?;
    if !status.success() {
        let default_nix = project_dir.join("default.nix");
        dump_with_lines(&default_nix)?;
        bail!(
            "nix-build {}\n=> exited with: {}",
            project_dir_display,
            status.code().unwrap_or(-1)
        );
    }
    eprintln!("Built {} successfully.", project_dir_display);

    Ok(())
}

/// Dump the content of the specified file with line numbers to stdout.
pub fn dump_with_lines(file_path: impl AsRef<Path>) -> Result<(), Error> {
    use std::io::{BufRead, Write};

    let file = std::fs::File::open(file_path.as_ref())?;
    let content = std::io::BufReader::new(file);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for (idx, line) in content.lines().enumerate() {
        writeln!(handle, "{:>5}: {}", idx + 1, line?)?;
    }

    Ok(())
}
