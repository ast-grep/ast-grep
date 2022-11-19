use crate::error::ErrorContext as EC;
use anyhow::{anyhow, Context, Result};
use crossterm::{
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ignore::{DirEntry, WalkParallel, WalkState};
use rprompt::prompt_reply_stdout;
use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;

// https://github.com/console-rs/console/blob/be1c2879536c90ffc2b54938b5964084f5fef67d/src/common_term.rs#L56
// clear screen
fn clear() {
  print!("\r\x1b[2J\r\x1b[H");
}

pub fn run_in_alternate_screen<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
  execute!(stdout(), EnterAlternateScreen)?;
  clear();
  let ret = f();
  execute!(stdout(), LeaveAlternateScreen)?;
  ret
}

pub fn prompt(prompt_text: &str, letters: &str, default: Option<char>) -> Result<char> {
  loop {
    let input = prompt_reply_stdout(prompt_text)?;
    if let Some(default) = default {
      if input.is_empty() {
        return Ok(default);
      }
    }
    if input.len() == 1 && letters.contains(&input) {
      return Ok(input.chars().next().unwrap());
    }
    println!("Unrecognized command, try again?")
  }
}

pub fn run_walker(walker: WalkParallel, f: impl Fn(DirEntry) -> WalkState + Sync) {
  walker.run(|| {
    Box::new(|result| match result {
      Ok(entry) => f(entry),
      Err(err) => {
        eprintln!("ERROR: {}", err);
        WalkState::Continue
      }
    })
  });
}

pub fn run_walker_interactive<T: Send>(
  walker: WalkParallel,
  producer: impl Fn(DirEntry) -> Option<T> + Sync,
  consumer: impl Fn(T) -> Result<()> + Send,
) -> Result<()> {
  let (tx, rx) = mpsc::channel();
  let producer = &producer;
  crossbeam::scope(|s| {
    s.spawn(move |_| {
      walker.run(|| {
        let tx = tx.clone();
        Box::new(move |result| {
          let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
              eprintln!("ERROR: {}", err);
              return WalkState::Continue;
            }
          };
          let result = match producer(entry) {
            Some(ret) => ret,
            None => return WalkState::Continue,
          };
          match tx.send(result) {
            Ok(_) => WalkState::Continue,
            Err(_) => WalkState::Quit,
          }
        })
      })
    });
    let interaction = s.spawn(move |_| -> Result<()> {
      while let Ok(match_result) = rx.recv() {
        consumer(match_result)?;
      }
      Ok(())
    });
    interaction
      .join()
      .expect("Error occurred during interaction.")
  })
  .expect("Error occurred during spawning threads")
}

pub fn open_in_editor(path: &PathBuf, start_line: usize) -> Result<()> {
  let editor = std::env::var("EDITOR").unwrap_or_else(|_| String::from("vim"));
  let exit = std::process::Command::new(editor)
    .arg(path)
    .arg(format!("+{}", start_line))
    .spawn()
    .context(EC::OpenEditor)?
    .wait()
    .context(EC::OpenEditor)?;
  if exit.success() {
    println!("wtf!!! {:?}", exit.code());
    Ok(())
  } else {
    println!("wtf!!! {:?}", exit.code());
    Err(anyhow!(EC::OpenEditor))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_open_editor() {
    // these two tests must run in sequence
    // since setting env will cause racing condition
    test_open_editor_respect_editor_env();
    test_open_editor_error_handling();
  }

  fn test_open_editor_respect_editor_env() {
    std::env::set_var("EDITOR", "echo");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    assert!(exit.is_ok());
  }

  fn test_open_editor_error_handling() {
    std::env::set_var("EDITOR", "NOT_EXIST_XXXXX");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    let error = exit.expect_err("should be error");
    let error = error.downcast_ref::<EC>().expect("should be error context");
    assert!(matches!(error, EC::OpenEditor));
  }
}
