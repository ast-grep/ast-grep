// The alias command `sg` redirects everything to ast-grep
// we need this to avoid "multiple build target" warning
// See https://github.com/rust-lang/cargo/issues/5930
fn main() -> std::io::Result<()> {
  // redirect to ast-grep
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
