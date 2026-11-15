# Command reference

Generated from cnpg-watch 0.1.0. Run `cnpg-watch command-reference` to regenerate.

## `cnpg-watch`

```text
Observe CloudNativePG topology and PostgreSQL runtime state.
Watch mode is read-only. Bootstrap operations belong to explicit setup commands.

Usage: cnpg-watch [OPTIONS] [COMMAND]

Commands:
  completions        Generate shell completions without reading config or contacting Kubernetes
  command-reference  Print the Markdown command reference without reading config or contacting Kubernetes
  watch              Open the observe-only dashboard (also the default command)
  info               Print paths and local configuration without contacting Kubernetes
  doctor             Check scoped access and the actual monitoring role without bootstrap changes
  setup              Review or explicitly apply runtime RBAC and PostgreSQL roles
  help               Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```

## `cnpg-watch completions`

```text
Generate shell completions without reading config or contacting Kubernetes

Usage: cnpg-watch completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>
          [possible values: bash, elvish, fish, powershell, zsh]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch command-reference`

```text
Print the Markdown command reference without reading config or contacting Kubernetes

Usage: cnpg-watch command-reference [OPTIONS]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch watch`

```text
Open the observe-only dashboard (also the default command)

Usage: cnpg-watch watch [OPTIONS]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch info`

```text
Print paths and local configuration without contacting Kubernetes

Usage: cnpg-watch info [OPTIONS]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --output <OUTPUT>
          [default: text]
          [possible values: text, json]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch doctor`

```text
Check scoped access and the actual monitoring role without bootstrap changes

Usage: cnpg-watch doctor [OPTIONS]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --output <OUTPUT>
          [default: text]
          [possible values: text, json]

      --include-cluster-resources
          Also check optional catalog/webhook object access

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch setup`

```text
Review or explicitly apply runtime RBAC and PostgreSQL roles

Usage: cnpg-watch setup [OPTIONS] <COMMAND>

Commands:
  kubernetes-rbac  
  postgres-role    
  pooler-role      
  help             Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch setup kubernetes-rbac`

```text
Usage: cnpg-watch setup kubernetes-rbac [OPTIONS] <--service-account <SERVICE_ACCOUNT>|--user <USER>|--group <GROUP>>

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --service-account <SERVICE_ACCOUNT>
          

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

      --service-account-namespace <SERVICE_ACCOUNT_NAMESPACE>
          

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --user <USER>
          

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --group <GROUP>
          

      --cluster <CLUSTER>
          

      --name <NAME>
          [default: cnpg-watch]

      --include-cluster-resources
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --dry-run
          

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --diff
          

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --apply
          

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --prune
          

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --output <OUTPUT>
          [default: yaml]
          [possible values: yaml, json, text]

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch setup postgres-role`

```text
Usage: cnpg-watch setup postgres-role [OPTIONS]

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --role-profile <ROLE_PROFILE>
          [default: monitor]
          [possible values: monitor, monitor-reserved, actions]

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

      --role-name <ROLE_NAME>
          

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --print-sql
          

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --dry-run
          

      --cluster <CLUSTER>
          

      --diff
          

      --apply
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --prune
          

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --preserve-password
          Preserve existing password (the default); new roles have no password

      --pooler-socket <POOLER_SOCKET>
          

      --rotate-password
          Set the password from the protected global --password-file; never print it

      --admin-role <ADMIN_ROLE>
          Explicit administrative login for setup only; never used by watch

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --admin-password-file <ADMIN_PASSWORD_FILE>
          

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --validate
          After apply, authenticate as the monitor and run the shipped fast bundle

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --output <OUTPUT>
          [default: sql]
          [possible values: sql, json, text]

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```

## `cnpg-watch setup pooler-role`

```text
Usage: cnpg-watch setup pooler-role [OPTIONS] --pooler <POOLER>

Options:
      --config <CONFIG>
          [env: CNPG_WATCH_CONFIG]

      --pooler <POOLER>
          

      --dry-run
          

      --kube-context <KUBE_CONTEXT>
          [env: CNPG_WATCH_CONTEXT]

      --diff
          

  -n, --namespace <NAMESPACE>
          [env: CNPG_WATCH_NAMESPACE]

      --all-namespaces[=<ALL_NAMESPACES>]
          [env: CNPG_WATCH_ALL_NAMESPACES]
          [possible values: true, false]

      --output <OUTPUT>
          [default: text]
          [possible values: text, json]

      --cluster <CLUSTER>
          

      --selector <SELECTOR>
          Kubernetes label selector for Cluster resources

      --show-query-text
          Display bounded query text in incident wait diagnostics; dumps always redact it

      --pooler-stats
          Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console

      --pooler-socket <POOLER_SOCKET>
          

      --cluster-resources
          Observe optional cluster-scoped webhook and image catalog metadata

      --certificate-secret <CERTIFICATE_SECRET>
          Explicit named certificate Secret reads (namespace/name); no Secret lists/watches

      --operator-metrics-url <OPERATOR_METRICS_URL>
          Optional bounded metrics scrape; disabled unless a URL is supplied

      --operator-namespace <OPERATOR_NAMESPACE>
          

      --operator-selector <OPERATOR_SELECTOR>
          

      --no-operator-watch
          

      --profile <PROFILE>
          [env: CNPG_WATCH_PROFILE]
          [possible values: minimal, standard, incident, secure]

      --refresh <REFRESH>
          [env: CNPG_WATCH_REFRESH]

      --password-file <PASSWORD_FILE>
          [env: CNPG_WATCH_PASSWORD_FILE]

      --log-file <LOG_FILE>
          [env: CNPG_WATCH_LOG_FILE]

      --dump-dir <DUMP_DIR>
          [env: CNPG_WATCH_DUMP_DIR]

      --db-role <DB_ROLE>
          [env: CNPG_WATCH_DB_ROLE]

      --verbose
          

  -h, --help
          Print help

```
