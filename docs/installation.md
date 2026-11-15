# Install cnpg-watch

Choose the archive for your operating system and CPU. Packages contain one
binary, five shell completion files, the generated command reference, and the
operator guides. Linux packages use musl and do not require a system Rust
installation. Package creation does not publish a release.

| Package suffix | Rust target |
| --- | --- |
| `darwin-arm64` | `aarch64-apple-darwin` |
| `darwin-amd64` | `x86_64-apple-darwin` |
| `linux-amd64` | `x86_64-unknown-linux-musl` |
| `linux-arm64` | `aarch64-unknown-linux-musl` |

Verify the archive against its accompanying `SHA256SUMS`, extract it, and copy
`cnpg-watch` into a directory on your PATH. On Linux use `sha256sum -c SHA256SUMS`;
on macOS use `shasum -a 256 -c SHA256SUMS`. Keep the package manifest if you need
to identify the binary and source hashes later.

```sh
mkdir -p "$HOME/.local/bin"
tar -xzf cnpg-watch-0.1.0-darwin-arm64.tar.gz
cp cnpg-watch-0.1.0-darwin-arm64/cnpg-watch "$HOME/.local/bin/"
cnpg-watch --help
cnpg-watch info --output json
```

Use the suffix matching your machine. These are command-line archives, without
an installer or platform notarization. The [compatibility guide](compatibility.md)
and [Phase 7 receipt](validation/phase-7.md) distinguish runtime smoke results
from release publication and hosted-CI qualification.

For Bash, source `completions/cnpg-watch.bash`; for Zsh, put `_cnpg-watch` in a
directory on `$fpath` and initialize completion with `compinit`. Fish users can
copy `cnpg-watch.fish` to their user completions directory. You can also regenerate
completions with `cnpg-watch completions bash`, `zsh`, `fish`, `powershell`, or
`elvish`. `cnpg-watch command-reference` prints the reference from the installed
binary. Both commands work without config or Kubernetes access.

Start with your current kubeconfig context and a namespace:

```sh
cnpg-watch --namespace app-db --profile minimal
cnpg-watch --namespace app-db doctor --output json
```

Minimal profile observes topology without SQL, logs, Secret reads, or optional
scrapes. To add SQL, provision the dedicated monitoring role through the
[explicit setup workflow](setup.md), protect its local password file with mode
0600, and select `--password-file`. The dashboard never bootstraps roles or RBAC.
See [operational evidence](operational.md) for PgBouncer, certificates and metrics.

At 120×40 the cluster dashboard shows health, instances, poolers and events.
At 100×30 it shows one secondary pane; select it with `:pane events` or
`:pane poolers`. Smaller terminals use a table-first layout. Press `i` for a
scrollable row snapshot, Enter for a related view, Esc/Left to return, Right to
go forward, and `?` for contextual keys. A snapshot is frozen and labelled with
its scope and capture time; `i` refreshes it. Scope changes discard it.

Diagnostics stay in the private bounded log selected by `--log-file`.
`info --output json` reports paths and limits. `:dump` writes an owner-only,
versioned artifact under `--dump-dir`, capped at 2 MiB. Dumps exclude API bodies,
credentials, SQL text, certificate bytes, metric labels and log/event messages.
