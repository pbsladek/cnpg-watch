# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_cnpg_watch_global_optspecs
    string join \n config= kube-context= n/namespace= all-namespaces= cluster= selector= show-query-text pooler-stats pooler-socket= cluster-resources certificate-secret= operator-metrics-url= operator-namespace= operator-selector= no-operator-watch profile= refresh= password-file= log-file= dump-dir= db-role= verbose h/help V/version
end

function __fish_cnpg_watch_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_cnpg_watch_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_cnpg_watch_using_subcommand
    set -l cmd (__fish_cnpg_watch_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -s V -l version -d 'Print version'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "completions" -d 'Generate shell completions without reading config or contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "command-reference" -d 'Print the Markdown command reference without reading config or contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "watch" -d 'Open the observe-only dashboard (also the default command)'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "info" -d 'Print paths and local configuration without contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "doctor" -d 'Check scoped access and the actual monitoring role without bootstrap changes'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "setup" -d 'Review or explicitly apply runtime RBAC and PostgreSQL roles'
complete -c cnpg-watch -n "__fish_cnpg_watch_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand completions" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand command-reference" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand watch" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l output -r -f -a "text\t''
json\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand info" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l output -r -f -a "text\t''
json\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l include-cluster-resources -d 'Also check optional catalog/webhook object access'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -f -a "kubernetes-rbac"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -f -a "postgres-role"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -f -a "pooler-role"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and not __fish_seen_subcommand_from kubernetes-rbac postgres-role pooler-role help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l service-account -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l service-account-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l user -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l group -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l name -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l output -r -f -a "yaml\t''
json\t''
text\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l include-cluster-resources
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l dry-run
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l diff
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l apply
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l prune
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from kubernetes-rbac" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l role-profile -r -f -a "monitor\t''
monitor-reserved\t''
actions\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l role-name -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l admin-role -d 'Explicit administrative login for setup only; never used by watch' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l admin-password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l output -r -f -a "sql\t''
json\t''
text\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l print-sql
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l dry-run
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l diff
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l apply
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l prune
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l preserve-password -d 'Preserve existing password (the default); new roles have no password'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l rotate-password -d 'Set the password from the protected global --password-file; never print it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l validate -d 'After apply, authenticate as the monitor and run the shipped fast bundle'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from postgres-role" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l pooler -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l output -r -f -a "text\t''
json\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l config -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l kube-context -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -s n -l namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l all-namespaces -r -f -a "true\t''
false\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l cluster -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l selector -d 'Kubernetes label selector for Cluster resources' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l pooler-socket -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l certificate-secret -d 'Explicit named certificate Secret reads (namespace/name); no Secret lists/watches' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l operator-metrics-url -d 'Optional bounded metrics scrape; disabled unless a URL is supplied' -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l operator-namespace -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l operator-selector -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l profile -r -f -a "minimal\t''
standard\t''
incident\t''
secure\t''"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l refresh -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l password-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l log-file -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l dump-dir -r -F
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l db-role -r
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l dry-run
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l diff
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l show-query-text -d 'Display bounded query text in incident wait diagnostics; dumps always redact it'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l pooler-stats -d 'Read fixed SHOW commands over CNPG\'s built-in local-peer PgBouncer console'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l cluster-resources -d 'Observe optional cluster-scoped webhook and image catalog metadata'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l no-operator-watch
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -l verbose
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from pooler-role" -s h -l help -d 'Print help'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from help" -f -a "kubernetes-rbac"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from help" -f -a "postgres-role"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from help" -f -a "pooler-role"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand setup; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "completions" -d 'Generate shell completions without reading config or contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "command-reference" -d 'Print the Markdown command reference without reading config or contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "watch" -d 'Open the observe-only dashboard (also the default command)'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "info" -d 'Print paths and local configuration without contacting Kubernetes'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "doctor" -d 'Check scoped access and the actual monitoring role without bootstrap changes'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "setup" -d 'Review or explicitly apply runtime RBAC and PostgreSQL roles'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and not __fish_seen_subcommand_from completions command-reference watch info doctor setup help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and __fish_seen_subcommand_from setup" -f -a "kubernetes-rbac"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and __fish_seen_subcommand_from setup" -f -a "postgres-role"
complete -c cnpg-watch -n "__fish_cnpg_watch_using_subcommand help; and __fish_seen_subcommand_from setup" -f -a "pooler-role"
