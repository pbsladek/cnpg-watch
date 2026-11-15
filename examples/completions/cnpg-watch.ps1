
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'cnpg-watch' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'cnpg-watch'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'cnpg-watch' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate shell completions without reading config or contacting Kubernetes')
            [CompletionResult]::new('command-reference', 'command-reference', [CompletionResultType]::ParameterValue, 'Print the Markdown command reference without reading config or contacting Kubernetes')
            [CompletionResult]::new('watch', 'watch', [CompletionResultType]::ParameterValue, 'Open the observe-only dashboard (also the default command)')
            [CompletionResult]::new('info', 'info', [CompletionResultType]::ParameterValue, 'Print paths and local configuration without contacting Kubernetes')
            [CompletionResult]::new('doctor', 'doctor', [CompletionResultType]::ParameterValue, 'Check scoped access and the actual monitoring role without bootstrap changes')
            [CompletionResult]::new('setup', 'setup', [CompletionResultType]::ParameterValue, 'Review or explicitly apply runtime RBAC and PostgreSQL roles')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'cnpg-watch;completions' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;command-reference' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;watch' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;info' {
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;doctor' {
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--include-cluster-resources', '--include-cluster-resources', [CompletionResultType]::ParameterName, 'Also check optional catalog/webhook object access')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;setup' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('kubernetes-rbac', 'kubernetes-rbac', [CompletionResultType]::ParameterValue, 'kubernetes-rbac')
            [CompletionResult]::new('postgres-role', 'postgres-role', [CompletionResultType]::ParameterValue, 'postgres-role')
            [CompletionResult]::new('pooler-role', 'pooler-role', [CompletionResultType]::ParameterValue, 'pooler-role')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'cnpg-watch;setup;kubernetes-rbac' {
            [CompletionResult]::new('--service-account', '--service-account', [CompletionResultType]::ParameterName, 'service-account')
            [CompletionResult]::new('--service-account-namespace', '--service-account-namespace', [CompletionResultType]::ParameterName, 'service-account-namespace')
            [CompletionResult]::new('--user', '--user', [CompletionResultType]::ParameterName, 'user')
            [CompletionResult]::new('--group', '--group', [CompletionResultType]::ParameterName, 'group')
            [CompletionResult]::new('--name', '--name', [CompletionResultType]::ParameterName, 'name')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--include-cluster-resources', '--include-cluster-resources', [CompletionResultType]::ParameterName, 'include-cluster-resources')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'dry-run')
            [CompletionResult]::new('--diff', '--diff', [CompletionResultType]::ParameterName, 'diff')
            [CompletionResult]::new('--apply', '--apply', [CompletionResultType]::ParameterName, 'apply')
            [CompletionResult]::new('--prune', '--prune', [CompletionResultType]::ParameterName, 'prune')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;setup;postgres-role' {
            [CompletionResult]::new('--role-profile', '--role-profile', [CompletionResultType]::ParameterName, 'role-profile')
            [CompletionResult]::new('--role-name', '--role-name', [CompletionResultType]::ParameterName, 'role-name')
            [CompletionResult]::new('--admin-role', '--admin-role', [CompletionResultType]::ParameterName, 'Explicit administrative login for setup only; never used by watch')
            [CompletionResult]::new('--admin-password-file', '--admin-password-file', [CompletionResultType]::ParameterName, 'admin-password-file')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--print-sql', '--print-sql', [CompletionResultType]::ParameterName, 'print-sql')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'dry-run')
            [CompletionResult]::new('--diff', '--diff', [CompletionResultType]::ParameterName, 'diff')
            [CompletionResult]::new('--apply', '--apply', [CompletionResultType]::ParameterName, 'apply')
            [CompletionResult]::new('--prune', '--prune', [CompletionResultType]::ParameterName, 'prune')
            [CompletionResult]::new('--preserve-password', '--preserve-password', [CompletionResultType]::ParameterName, 'Preserve existing password (the default); new roles have no password')
            [CompletionResult]::new('--rotate-password', '--rotate-password', [CompletionResultType]::ParameterName, 'Set the password from the protected global --password-file; never print it')
            [CompletionResult]::new('--validate', '--validate', [CompletionResultType]::ParameterName, 'After apply, authenticate as the monitor and run the shipped fast bundle')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;setup;pooler-role' {
            [CompletionResult]::new('--pooler', '--pooler', [CompletionResultType]::ParameterName, 'pooler')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config')
            [CompletionResult]::new('--kube-context', '--kube-context', [CompletionResultType]::ParameterName, 'kube-context')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--all-namespaces', '--all-namespaces', [CompletionResultType]::ParameterName, 'all-namespaces')
            [CompletionResult]::new('--cluster', '--cluster', [CompletionResultType]::ParameterName, 'cluster')
            [CompletionResult]::new('--selector', '--selector', [CompletionResultType]::ParameterName, 'Kubernetes label selector for Cluster resources')
            [CompletionResult]::new('--pooler-socket', '--pooler-socket', [CompletionResultType]::ParameterName, 'pooler-socket')
            [CompletionResult]::new('--certificate-secret', '--certificate-secret', [CompletionResultType]::ParameterName, 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches')
            [CompletionResult]::new('--operator-metrics-url', '--operator-metrics-url', [CompletionResultType]::ParameterName, 'Optional bounded metrics scrape; disabled unless a URL is supplied')
            [CompletionResult]::new('--operator-namespace', '--operator-namespace', [CompletionResultType]::ParameterName, 'operator-namespace')
            [CompletionResult]::new('--operator-selector', '--operator-selector', [CompletionResultType]::ParameterName, 'operator-selector')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'profile')
            [CompletionResult]::new('--refresh', '--refresh', [CompletionResultType]::ParameterName, 'refresh')
            [CompletionResult]::new('--password-file', '--password-file', [CompletionResultType]::ParameterName, 'password-file')
            [CompletionResult]::new('--log-file', '--log-file', [CompletionResultType]::ParameterName, 'log-file')
            [CompletionResult]::new('--dump-dir', '--dump-dir', [CompletionResultType]::ParameterName, 'dump-dir')
            [CompletionResult]::new('--db-role', '--db-role', [CompletionResultType]::ParameterName, 'db-role')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'dry-run')
            [CompletionResult]::new('--diff', '--diff', [CompletionResultType]::ParameterName, 'diff')
            [CompletionResult]::new('--show-query-text', '--show-query-text', [CompletionResultType]::ParameterName, 'Display bounded query text in incident wait diagnostics; dumps always redact it')
            [CompletionResult]::new('--pooler-stats', '--pooler-stats', [CompletionResultType]::ParameterName, 'Read fixed SHOW commands over CNPG''s built-in local-peer PgBouncer console')
            [CompletionResult]::new('--cluster-resources', '--cluster-resources', [CompletionResultType]::ParameterName, 'Observe optional cluster-scoped webhook and image catalog metadata')
            [CompletionResult]::new('--no-operator-watch', '--no-operator-watch', [CompletionResultType]::ParameterName, 'no-operator-watch')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'verbose')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'cnpg-watch;setup;help' {
            [CompletionResult]::new('kubernetes-rbac', 'kubernetes-rbac', [CompletionResultType]::ParameterValue, 'kubernetes-rbac')
            [CompletionResult]::new('postgres-role', 'postgres-role', [CompletionResultType]::ParameterValue, 'postgres-role')
            [CompletionResult]::new('pooler-role', 'pooler-role', [CompletionResultType]::ParameterValue, 'pooler-role')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'cnpg-watch;setup;help;kubernetes-rbac' {
            break
        }
        'cnpg-watch;setup;help;postgres-role' {
            break
        }
        'cnpg-watch;setup;help;pooler-role' {
            break
        }
        'cnpg-watch;setup;help;help' {
            break
        }
        'cnpg-watch;help' {
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate shell completions without reading config or contacting Kubernetes')
            [CompletionResult]::new('command-reference', 'command-reference', [CompletionResultType]::ParameterValue, 'Print the Markdown command reference without reading config or contacting Kubernetes')
            [CompletionResult]::new('watch', 'watch', [CompletionResultType]::ParameterValue, 'Open the observe-only dashboard (also the default command)')
            [CompletionResult]::new('info', 'info', [CompletionResultType]::ParameterValue, 'Print paths and local configuration without contacting Kubernetes')
            [CompletionResult]::new('doctor', 'doctor', [CompletionResultType]::ParameterValue, 'Check scoped access and the actual monitoring role without bootstrap changes')
            [CompletionResult]::new('setup', 'setup', [CompletionResultType]::ParameterValue, 'Review or explicitly apply runtime RBAC and PostgreSQL roles')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'cnpg-watch;help;completions' {
            break
        }
        'cnpg-watch;help;command-reference' {
            break
        }
        'cnpg-watch;help;watch' {
            break
        }
        'cnpg-watch;help;info' {
            break
        }
        'cnpg-watch;help;doctor' {
            break
        }
        'cnpg-watch;help;setup' {
            [CompletionResult]::new('kubernetes-rbac', 'kubernetes-rbac', [CompletionResultType]::ParameterValue, 'kubernetes-rbac')
            [CompletionResult]::new('postgres-role', 'postgres-role', [CompletionResultType]::ParameterValue, 'postgres-role')
            [CompletionResult]::new('pooler-role', 'pooler-role', [CompletionResultType]::ParameterValue, 'pooler-role')
            break
        }
        'cnpg-watch;help;setup;kubernetes-rbac' {
            break
        }
        'cnpg-watch;help;setup;postgres-role' {
            break
        }
        'cnpg-watch;help;setup;pooler-role' {
            break
        }
        'cnpg-watch;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
