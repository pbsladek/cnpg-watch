use std::process::Command;
#[test]
fn generated_cli_artifacts_are_offline_deterministic_and_hide_env_values() {
    for (args, artifact) in [
        (
            vec!["completions", "bash"],
            "examples/completions/cnpg-watch.bash",
        ),
        (
            vec!["completions", "zsh"],
            "examples/completions/_cnpg-watch",
        ),
        (
            vec!["completions", "fish"],
            "examples/completions/cnpg-watch.fish",
        ),
        (
            vec!["completions", "powershell"],
            "examples/completions/cnpg-watch.ps1",
        ),
        (
            vec!["completions", "elvish"],
            "examples/completions/cnpg-watch.elv",
        ),
        (vec!["command-reference"], "docs/command-reference.md"),
    ] {
        let run = || {
            Command::new(env!("CARGO_BIN_EXE_cnpg-watch"))
                .args(["--config", "/missing/does-not-exist"])
                .args(&args)
                .env("CNPG_WATCH_PASSWORD_FILE", "SECRET_PATH_MARKER")
                .output()
                .unwrap()
        };
        let a = run();
        let b = run();
        assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
        assert_eq!(a.stdout, b.stdout);
        assert_eq!(
            a.stdout,
            std::fs::read(artifact).unwrap(),
            "regenerate {artifact} with make cli-artifacts"
        );
        assert!(a.stderr.is_empty());
        let text = String::from_utf8(a.stdout).unwrap();
        assert!(!text.contains("SECRET_PATH_MARKER"));
        assert!(text.contains("cnpg-watch"));
        if args[0] == "command-reference" {
            for command in [
                "cnpg-watch doctor",
                "cnpg-watch setup pooler-role",
                "cnpg-watch setup kubernetes-rbac",
                "--certificate-secret",
            ] {
                assert!(text.contains(command));
            }
        }
    }
}
