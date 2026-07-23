// Microsoft Defender has quarantined the small Windows forwarding executable as
// Trojan:Win64/Lazy!MTB, breaking pip and downstream installs. Build `sg.exe` as
// the complete CLI to avoid that false positive. See #2799 and #2841.
#[cfg(windows)]
fn main() -> anyhow::Result<std::process::ExitCode> {
  ast_grep::execute_main()
}

// Keep `sg` as a lightweight launcher on Unix.
#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
  use std::env::args;
  use std::process::{Command, Stdio};
  let mut child = Command::new("ast-grep")
    .args(args().skip(1))
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()?;
  let status = child.wait()?;
  std::process::exit(status.code().unwrap_or(1))
}
