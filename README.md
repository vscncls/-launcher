# discord-launcher

A small launcher that keeps Discord's Linux tarball up to date in your home directory, so you don't need `pacman -Sy discord` (and therefore a partial-upgrade sync) every time Discord ships a new release.

Based on [kpcyrd/spotify-launcher](https://github.com/kpcyrd/spotify-launcher) — same approach, retargeted at Discord's stable tarball feed.

## How it works

- On launch, queries `https://discord.com/api/updates/stable?platform=linux` for the latest version.
- If the local install is stale (24h since last check, or version mismatch), downloads `discord-<version>.tar.gz` from `dl.discordapp.net`, extracts it into `~/.local/share/discord-launcher/install-new/`, and atomically swaps it with `install/`.
- Execs `~/.local/share/discord-launcher/install/Discord/Discord`, passing through any extra arguments/env vars from your config, plus the URI if one was given.

Verification is HTTPS-only — Discord doesn't publish PGP signatures or checksums for these tarballs, so we rely on TLS + DNS.

## Configuration

Config lookup order:

- `${XDG_CONFIG_HOME:-$HOME/.config}/discord-launcher.conf`
- `/etc/discord-launcher.conf`

```toml
[discord]
## Pass extra arguments to the Discord executable
#extra_arguments = []
## If unprivileged user namespaces are disabled, chrome-sandbox will fail
## (the launcher installs without root, so it can't set the setuid bit).
#extra_arguments = ["--no-sandbox"]
## How often to retry a resumed download before giving up (0 for unlimited)
#download_attempts = 5
```

## Invite links

The packaged desktop file registers `discord-launcher` as a handler for `x-scheme-handler/discord`, so `discord://-/invite/…` URLs opened from a browser launch Discord via the launcher. The URI is passed through as a positional argument to the Discord binary.

## CLI

```
discord-launcher [URI]
  --skip-update        Skip the update check
  --check-update       Always check for updates
  --force-update       Re-download even if already on latest
  --tarball PATH       Install from a local .tar.gz instead of downloading
  --install-dir PATH   Install into a specific directory
  --no-exec            Don't exec Discord after updating (for testing)
  --timeout SECS       HTTP timeout (0 for none)
  --download-attempts N
  -v / -vv             Verbose logs
```

## License

`Apache-2.0 OR MIT`
