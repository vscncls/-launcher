use crate::errors::*;
use crate::http;
use crate::progress::ProgressBar;
use serde::Deserialize;

pub const DEFAULT_DOWNLOAD_ATTEMPTS: usize = 5;
const UPDATE_API: &str = "https://discord.com/api/updates/stable?platform=linux";
const DOWNLOAD_BASE: &str = "https://dl.discordapp.net/apps/linux";

#[derive(Debug, Deserialize)]
struct UpdateApiResponse {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub_date: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub url: String,
}

pub struct Client {
    client: http::Client,
}

impl Client {
    pub fn new(timeout: Option<u64>) -> Result<Client> {
        let client = http::Client::new(timeout)?;
        Ok(Client { client })
    }

    pub async fn fetch_latest(&self) -> Result<Release> {
        info!("Querying Discord update API...");
        let body = self.client.fetch(UPDATE_API).await?;
        let resp: UpdateApiResponse = serde_json::from_slice(&body)
            .context("Failed to parse Discord update API response")?;

        let version = resp.name;
        if version.is_empty()
            || !version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            bail!("Discord API returned unexpected version: {:?}", version);
        }

        let url = resp
            .url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| format!("{DOWNLOAD_BASE}/{version}/discord-{version}.tar.gz"));

        debug!("Latest Discord release: version={version}, url={url}");
        Ok(Release { version, url })
    }

    async fn attempt_download(
        &self,
        url: &str,
        buf: &mut Vec<u8>,
        pb: &mut ProgressBar,
        offset: &mut Option<u64>,
    ) -> Result<()> {
        let mut dl = self.client.fetch_stream(url, *offset).await?;
        while let Some(chunk) = dl.chunk().await? {
            buf.extend(&chunk);
            *offset = Some(dl.progress);

            let progress = if dl.total > 0 {
                (dl.progress as f64 / dl.total as f64 * 100.0) as u64
            } else {
                0
            };
            pb.update(progress).await?;
            debug!(
                "Download progress: {}%, {}/{}",
                progress, dl.progress, dl.total
            );
        }
        Ok(())
    }

    pub async fn download_tarball(
        &self,
        release: &Release,
        max_download_attempts: usize,
    ) -> Result<Vec<u8>> {
        info!(
            "Downloading Discord version={:?} from {:?}",
            release.version, release.url
        );

        let mut pb = ProgressBar::spawn()?;
        let mut buf = Vec::new();
        let mut offset = None;

        let mut i: usize = 0;
        loop {
            i = i.saturating_add(1);
            if max_download_attempts > 0 && i > max_download_attempts {
                break;
            }

            if i > 1 {
                info!("Retrying download...");
            }

            if let Err(err) = self
                .attempt_download(&release.url, &mut buf, &mut pb, &mut offset)
                .await
            {
                warn!("Download has failed: {err:#}");
            } else {
                pb.close().await?;
                return Ok(buf);
            }
        }

        pb.close().await?;
        bail!("Exceeded number of retries for download");
    }
}
