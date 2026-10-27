//! End-to-end checks against the built `tp` binary.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

struct Env {
    dir: PathBuf,
}

impl Env {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tp-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Env { dir }
    }

    fn tp(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tp"))
            .args(args)
            .env("TP_DATA", self.dir.join("habits.json"))
            .env("TP_TODAY", "2026-10-27")
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env_remove("NO_COLOR")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.tp(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn add_done_show() {
    let env = Env::new();
    assert_eq!(env.ok(&[]), "no habits yet — try: tp add read\n");
    env.ok(&["add", "water", "--target", "8", "--unit", "glasses"]);
    env.ok(&["add", "read"]);
    assert_eq!(
        env.ok(&["done", "wa", "5"]),
        "  water — 2026-10-27 · 5/8 glasses · streak 0\n"
    );
    assert_eq!(
        env.ok(&["done", "water", "--full"]),
        "✓ water — 2026-10-27 · 8/8 glasses · streak 1\n"
    );
    assert_eq!(
        env.ok(&["done", "read", "-d", "-1"]),
        "✓ read — 2026-10-26 · streak 1\n"
    );
    assert_eq!(env.ok(&["skip", "read"]), "· read skipped — 2026-10-27\n");
    let show = env.ok(&["show", "water", "--weeks", "4"]);
    assert!(!show.contains('\x1b'), "piped output must not be colored");
    assert!(show.contains("target 8 glasses"));
    assert!(show.contains("streak 1 · longest 1"));
    let list = env.ok(&["list"]);
    assert!(list.contains("water  8/8"), "{list}");
    assert!(list.contains("read   skip"), "{list}");
}

#[test]
fn json_output() {
    let env = Env::new();
    env.ok(&["add", "water", "--target", "8"]);
    env.ok(&["done", "water", "4"]);
    let list: serde_json::Value = serde_json::from_str(&env.ok(&["list", "--json"])).unwrap();
    assert_eq!(list["today"], "2026-10-27");
    assert_eq!(list["habits"][0]["today"]["value"], 4);
    let show: serde_json::Value =
        serde_json::from_str(&env.ok(&["show", "water", "--json"])).unwrap();
    assert_eq!(show["days"].as_array().unwrap().len(), 1);
    assert_eq!(show["days"][0]["state"], "pending");
}

#[test]
fn errors_and_exit_codes() {
    let env = Env::new();
    env.ok(&["add", "gym"]);
    env.ok(&["add", "guitar"]);
    let out = env.tp(&["done", "gym", "-d", "2026-10-28"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: 2026-10-28 is in the future\n"
    );
    let out = env.tp(&["done", "g"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: 'g' matches gym, guitar\n"
    );
    assert_eq!(env.tp(&["done", "gym", "3"]).status.code(), Some(1));
    assert_eq!(env.tp(&["bogus"]).status.code(), Some(2));
    assert_eq!(
        env.tp(&["done", "gym", "3", "--set", "1"]).status.code(),
        Some(2)
    );
    let out = env.tp(&["done", "gym", "-d", "2026-10-01"]);
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning: editing 2026-10-01 (26 days ago)"),
        "{stderr}"
    );
    assert!(
        stderr.contains("note: gym start moved to 2026-10-01"),
        "{stderr}"
    );
}

#[test]
fn delete_needs_force_without_tty() {
    let env = Env::new();
    env.ok(&["add", "gym"]);
    let out = env.tp(&["delete", "gym"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--force"));
    assert_eq!(
        env.ok(&["delete", "gym", "--force"]),
        "deleted gym (0 entries)\n"
    );
    assert_eq!(env.ok(&["list"]), "no habits yet — try: tp add read\n");
}

#[test]
fn corrupt_file_is_never_overwritten() {
    let env = Env::new();
    let path = env.dir.join("habits.json");
    std::fs::write(&path, "{ not json").unwrap();
    let out = env.tp(&["add", "read"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("line 1"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
}
