_cnpg__watch() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="cnpg__watch"
                ;;
            cnpg__watch,command-reference)
                cmd="cnpg__watch__subcmd__command__subcmd__reference"
                ;;
            cnpg__watch,completions)
                cmd="cnpg__watch__subcmd__completions"
                ;;
            cnpg__watch,doctor)
                cmd="cnpg__watch__subcmd__doctor"
                ;;
            cnpg__watch,help)
                cmd="cnpg__watch__subcmd__help"
                ;;
            cnpg__watch,info)
                cmd="cnpg__watch__subcmd__info"
                ;;
            cnpg__watch,setup)
                cmd="cnpg__watch__subcmd__setup"
                ;;
            cnpg__watch,watch)
                cmd="cnpg__watch__subcmd__watch"
                ;;
            cnpg__watch__subcmd__help,command-reference)
                cmd="cnpg__watch__subcmd__help__subcmd__command__subcmd__reference"
                ;;
            cnpg__watch__subcmd__help,completions)
                cmd="cnpg__watch__subcmd__help__subcmd__completions"
                ;;
            cnpg__watch__subcmd__help,doctor)
                cmd="cnpg__watch__subcmd__help__subcmd__doctor"
                ;;
            cnpg__watch__subcmd__help,help)
                cmd="cnpg__watch__subcmd__help__subcmd__help"
                ;;
            cnpg__watch__subcmd__help,info)
                cmd="cnpg__watch__subcmd__help__subcmd__info"
                ;;
            cnpg__watch__subcmd__help,setup)
                cmd="cnpg__watch__subcmd__help__subcmd__setup"
                ;;
            cnpg__watch__subcmd__help,watch)
                cmd="cnpg__watch__subcmd__help__subcmd__watch"
                ;;
            cnpg__watch__subcmd__help__subcmd__setup,kubernetes-rbac)
                cmd="cnpg__watch__subcmd__help__subcmd__setup__subcmd__kubernetes__subcmd__rbac"
                ;;
            cnpg__watch__subcmd__help__subcmd__setup,pooler-role)
                cmd="cnpg__watch__subcmd__help__subcmd__setup__subcmd__pooler__subcmd__role"
                ;;
            cnpg__watch__subcmd__help__subcmd__setup,postgres-role)
                cmd="cnpg__watch__subcmd__help__subcmd__setup__subcmd__postgres__subcmd__role"
                ;;
            cnpg__watch__subcmd__setup,help)
                cmd="cnpg__watch__subcmd__setup__subcmd__help"
                ;;
            cnpg__watch__subcmd__setup,kubernetes-rbac)
                cmd="cnpg__watch__subcmd__setup__subcmd__kubernetes__subcmd__rbac"
                ;;
            cnpg__watch__subcmd__setup,pooler-role)
                cmd="cnpg__watch__subcmd__setup__subcmd__pooler__subcmd__role"
                ;;
            cnpg__watch__subcmd__setup,postgres-role)
                cmd="cnpg__watch__subcmd__setup__subcmd__postgres__subcmd__role"
                ;;
            cnpg__watch__subcmd__setup__subcmd__help,help)
                cmd="cnpg__watch__subcmd__setup__subcmd__help__subcmd__help"
                ;;
            cnpg__watch__subcmd__setup__subcmd__help,kubernetes-rbac)
                cmd="cnpg__watch__subcmd__setup__subcmd__help__subcmd__kubernetes__subcmd__rbac"
                ;;
            cnpg__watch__subcmd__setup__subcmd__help,pooler-role)
                cmd="cnpg__watch__subcmd__setup__subcmd__help__subcmd__pooler__subcmd__role"
                ;;
            cnpg__watch__subcmd__setup__subcmd__help,postgres-role)
                cmd="cnpg__watch__subcmd__setup__subcmd__help__subcmd__postgres__subcmd__role"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        cnpg__watch)
            opts="-n -h -V --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help --version completions command-reference watch info doctor setup help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__command__subcmd__reference)
            opts="-n -h --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__completions)
            opts="-n -h --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help bash elvish fish powershell zsh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__doctor)
            opts="-n -h --output --include-cluster-resources --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help)
            opts="completions command-reference watch info doctor setup help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__command__subcmd__reference)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__completions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__doctor)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__info)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__setup)
            opts="kubernetes-rbac postgres-role pooler-role"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__setup__subcmd__kubernetes__subcmd__rbac)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__setup__subcmd__pooler__subcmd__role)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__setup__subcmd__postgres__subcmd__role)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__help__subcmd__watch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__info)
            opts="-n -h --output --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup)
            opts="-n -h --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help kubernetes-rbac postgres-role pooler-role help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__help)
            opts="kubernetes-rbac postgres-role pooler-role help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__help__subcmd__kubernetes__subcmd__rbac)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__help__subcmd__pooler__subcmd__role)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__help__subcmd__postgres__subcmd__role)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__kubernetes__subcmd__rbac)
            opts="-n -h --service-account --service-account-namespace --user --group --name --include-cluster-resources --dry-run --diff --apply --prune --output --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --service-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --service-account-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --user)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --group)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "yaml json text" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__pooler__subcmd__role)
            opts="-n -h --pooler --dry-run --diff --output --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --pooler)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__setup__subcmd__postgres__subcmd__role)
            opts="-n -h --role-profile --role-name --print-sql --dry-run --diff --apply --prune --preserve-password --rotate-password --admin-role --admin-password-file --validate --output --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --role-profile)
                    COMPREPLY=($(compgen -W "monitor monitor-reserved actions" -- "${cur}"))
                    return 0
                    ;;
                --role-name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --admin-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --admin-password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "sql json text" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        cnpg__subcmd__watch__subcmd__watch)
            opts="-n -h --config --kube-context --namespace --all-namespaces --cluster --selector --show-query-text --pooler-stats --pooler-socket --cluster-resources --certificate-secret --operator-metrics-url --operator-namespace --operator-selector --no-operator-watch --profile --refresh --password-file --log-file --dump-dir --db-role --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kube-context)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --all-namespaces)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --cluster)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pooler-socket)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --certificate-secret)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operator-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -W "minimal standard incident secure" -- "${cur}"))
                    return 0
                    ;;
                --refresh)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --db-role)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _cnpg__watch -o nosort -o bashdefault -o default cnpg-watch
else
    complete -F _cnpg__watch -o bashdefault -o default cnpg-watch
fi
