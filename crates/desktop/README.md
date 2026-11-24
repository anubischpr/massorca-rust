# MassOrca desktop

The desktop window talks to the service of one profile. This page covers what it leaves on disk to help debug a problem.

## Local log

The desktop writes its own log, separate from the service's log:

- File: `<home>/profiles/<name>/logs/desktop.<YYYY-MM-DD>.log`, one file per day. The newest seven are kept. The service
  writes `service.<YYYY-MM-DD>.log` in the same folder.
- `<home>` is `--home` or `MASSORCA_HOME`, else the default MassOrca data folder. `<name>` is `--profile` or
  `MASSORCA_PROFILE`, else `default`.
- Level: `MASSORCA_LOG`, in `tracing` filter syntax, `info` when unset. For example, `MASSORCA_LOG=debug`.
- Redaction: values under keys such as `token`, `password`, `secret`, `api_key` and `authorization` are written as
  `[redacted]`. Frame dumps from the transport are never written.
- Nothing is sent anywhere. The file stays in the profile folder.
- No CLI command prints this path, so the folder above is the entry point.
