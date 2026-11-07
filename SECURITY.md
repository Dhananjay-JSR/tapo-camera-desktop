# Security

## What is and isn't a secret

- **Not secrets:** the `ACCESS_KEY` and `SECRET` constants in `crates/tapo-core/src/sign.rs` and the
  bundled `tplink_ca.pem`. These are client values shipped inside the official Tapo app (identical
  for every install) and are required to talk to the cloud. They are safe to commit.
- **Secrets (never commit):** your TP-Link account token / session. It is written only to your OS
  config directory (`session.json`), created with `0600` permissions, and masked in the log. The
  `.gitignore` also blocks `session.json`, `*.flow` and `*.ts`.

When filing an issue, attach the log from **Log > Open log file**. It already masks tokens; do not
paste raw tokens or camera footage.

## Reporting a vulnerability

Open a private security advisory on the GitHub repository, or a regular issue for non-sensitive
reports.
