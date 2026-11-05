//! The offline-only build from the outside: with no account signed in at all, neither
//! `horizon-launcher play` nor the MCP server's play asks for a Microsoft account. Nothing here
//! downloads Minecraft either, so without game files to prepare they fail for want of those,
//! never for want of an account.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The refusal the owner gate used to give. No path may produce it any more.
const REFUSED: &str = "Sign in with a Microsoft account that owns Minecraft: Java Edition to play.";

/// Fresh homes with one installed (not prepared) version and an offline name, and no account.
fn home(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("horizon-offline-only-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let version = root.join("launcher/versions/v2026.10.27.01");
    std::fs::create_dir_all(&version).unwrap();
    std::fs::write(version.join(".installed"), "").unwrap();
    std::fs::write(
        root.join("launcher/accounts.json"),
        r#"{"accounts":[],"selected":null,"offline":["Builder_1"],"offline_selected":"Builder_1"}"#,
    )
    .unwrap();
    root
}

fn launcher(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_horizon-launcher"));
    cmd.args(args)
        .env("HORIZON_HOME", root.join("game"))
        .env("HORIZON_LAUNCHER_HOME", root.join("launcher"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("HORIZON_NO_AUDIO", "1")
        .env("HORIZON_TELEMETRY_URL", "off")
        // Memory only: the test never reads the player's keychain.
        .env("HORIZON_NO_KEYCHAIN", "1")
        .env("HORIZON_MSA_CLIENT_ID", "test-client");
    cmd
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn the_cli_never_asks_for_an_account() {
    let root = home("cli");
    let out = launcher(&root, &["play"]).output().unwrap();
    let said = text(&out);
    assert!(!said.contains(REFUSED), "{said}");
    assert!(!said.contains("horizon-launcher account sign-in"), "{said}");
    // No session is ever written: an offline name hands the game nothing.
    assert!(!root.join("game/sessions").exists());
    // The offline name is still the one that plays.
    let accounts = std::fs::read_to_string(root.join("launcher/accounts.json")).unwrap();
    assert!(accounts.contains("Builder_1"), "{accounts}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_cli_never_downloads_minecraft() {
    let root = home("download");
    let out = launcher(&root, &["play"]).output().unwrap();
    let said = text(&out);
    // Nothing claims to fetch the game from Mojang; the version was installed, not prepared,
    // so the refusal is about game files, and it names where to point the launcher instead.
    assert!(!said.contains("downloading"), "{said}");
    assert!(!said.contains("from Mojang"), "{said}");
    assert!(!out.status.success(), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_mcp_server_never_asks_for_an_account() {
    let root = home("mcp");
    let mut child = launcher(&root, &["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"play","arguments":{}}}"#,
    ];
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let reply = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["id"] == 2)
        .expect("a reply to play");
    let said = reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(!said.contains(REFUSED), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}
