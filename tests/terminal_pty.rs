#[path = "support/pty.rs"]
mod pty;
#[test]
#[ignore = "make test-terminal builds the lifecycle fixture first"]
fn portable_pty_vt100_restores_guard_on_q_interrupt_error_and_panic() {
    for (key, status) in [(b'q', 0), (3, 0), (b'e', 1), (b'p', 101)] {
        let mut command = portable_pty::CommandBuilder::new(
            std::env::current_dir()
                .unwrap()
                .join("target/debug/examples/terminal_lifecycle"),
        );
        command.env("TERM", "xterm-256color");
        let mut child = pty::Pty::spawn(command, 120, 40);
        child.wait("READY", 10);
        assert!(!child.restored());
        child.resize(100, 30);
        child.send(&[key]);
        assert_eq!(child.exit(5), status);
        assert!(child.restored());
    }
}
