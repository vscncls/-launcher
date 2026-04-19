use clap::Parser;
use discord_launcher::args::Args;
use discord_launcher::config::ConfigFile;
use discord_launcher::discord;
use discord_launcher::discord::Client;
use discord_launcher::errors::*;
use discord_launcher::extract;
use discord_launcher::paths;
use discord_launcher::ui;
use env_logger::Env;
use std::ffi::CString;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;
use tokio::fs;

const UPDATE_CHECK_INTERVAL: u64 = 3600 * 24;

struct VersionCheck {
    tarball: Option<Vec<u8>>,
    version: String,
}

async fn should_update(args: &Args, state: Option<&paths::State>) -> Result<bool> {
    if args.force_update || args.check_update || args.tarball.is_some() {
        Ok(true)
    } else if args.skip_update {
        Ok(false)
    } else if let Some(state) = &state {
        let Ok(since_update) = SystemTime::now().duration_since(state.last_update_check) else {
            return Ok(true);
        };

        let hours_since = since_update.as_secs() / 3600;
        let days_since = hours_since / 24;
        let hours_since = hours_since % 24;

        debug!(
            "Last update check was {} days and {} hours ago",
            days_since, hours_since
        );
        Ok(since_update >= Duration::from_secs(UPDATE_CHECK_INTERVAL))
    } else {
        Ok(true)
    }
}

async fn update(
    args: &Args,
    state: Option<&paths::State>,
    install_path: &Path,
    download_attempts: usize,
) -> Result<()> {
    let update = if let Some(tarball_path) = &args.tarball {
        let tarball = fs::read(tarball_path)
            .await
            .with_context(|| anyhow!("Failed to read tarball from {:?}", tarball_path))?;
        VersionCheck {
            tarball: Some(tarball),
            version: "0".to_string(),
        }
    } else {
        let client = Client::new(args.timeout)?;
        let release = client.fetch_latest().await?;

        match state {
            Some(state) if state.version == release.version && !args.force_update => {
                info!("Latest version is already installed, not updating");
                VersionCheck {
                    tarball: None,
                    version: release.version,
                }
            }
            _ => {
                let tarball = client.download_tarball(&release, download_attempts).await?;
                VersionCheck {
                    tarball: Some(tarball),
                    version: release.version,
                }
            }
        }
    };

    if let Some(tarball) = update.tarball {
        extract::pkg(&tarball[..], args, install_path).await?;
    }

    debug!("Updating state file");
    let buf = serde_json::to_string(&paths::State {
        last_update_check: SystemTime::now(),
        version: update.version,
    })?;
    fs::write(paths::state_file_path()?, buf)
        .await
        .context("Failed to write state file")?;

    Ok(())
}

fn start(args: &Args, cf: &ConfigFile, install_path: &Path) -> Result<()> {
    let bin = install_path.join("Discord/Discord");
    let bin = CString::new(bin.to_string_lossy().as_bytes())?;

    let mut exec_args = vec![CString::new("Discord")?];

    for arg in cf.discord.extra_arguments.iter().cloned() {
        exec_args.push(CString::new(arg)?);
    }

    if let Some(uri) = &args.uri {
        exec_args.push(CString::new(uri.as_str())?);
    }

    debug!("Assembled command: {:?}", exec_args);

    if args.no_exec {
        info!("Skipping exec because --no-exec was used");
    } else {
        cf.discord.extra_env_vars.iter().for_each(|x| {
            let (k, v) = match x.split_once('=') {
                None => (x.as_str(), ""),
                Some(x) => x,
            };
            std::env::set_var(k, v);
        });
        nix::unistd::execv(&bin, &exec_args)
            .with_context(|| anyhow!("Failed to exec {:?}", bin))?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match args.verbose {
        0 => "info",
        1 => "info,discord_launcher=debug",
        2 => "debug",
        _ => "trace",
    };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    let cf = ConfigFile::load().context("Failed to load configuration")?;

    let install_path = if let Some(path) = &args.install_dir {
        path.clone()
    } else {
        paths::install_path()?
    };
    debug!("Using install path: {:?}", install_path);

    let download_attempts = args.download_attempts.unwrap_or_else(|| {
        cf.discord
            .download_attempts
            .unwrap_or(discord::DEFAULT_DOWNLOAD_ATTEMPTS)
    });

    let state = paths::load_state_file().await?;
    if should_update(&args, state.as_ref()).await? {
        if let Err(err) = update(&args, state.as_ref(), &install_path, download_attempts).await {
            error!("Update failed: {err:#}");
            ui::error(&err).await?;
        }
    } else {
        info!("No update needed");
    }
    start(&args, &cf, &install_path)?;

    Ok(())
}
