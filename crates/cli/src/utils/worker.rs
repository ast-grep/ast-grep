use crate::print::Printer;
use crate::utils::FileTrace;

use anyhow::{Result, anyhow};
use ignore::{DirEntry, WalkParallel, WalkState};

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

/// A trait to abstract how ast-grep discovers work Items.
///
/// It follows multiple-producer-single-consumer pattern.
/// ast-grep will produce items in one or more separate thread(s) and
/// `consume_items` in the main thread, blocking the function return.
/// Worker at the moment has two main flavors:
/// * PathWorker: discovers files on the file system, based on ignore
/// * StdInWorker: parse text content from standard input stream
pub trait Worker: Sync + Send {
  /// `consume_items` will run in a separate single thread.
  /// printing matches or error reporting can happen here.
  fn consume_items<P: Printer>(&self, items: Items<P::Processed>, printer: P) -> Result<ExitCode>;
}

/// A trait to abstract how ast-grep discovers, parses and processes files.
///
/// It follows multiple-producer-single-consumer pattern.
/// ast-grep discovers files in parallel by `build_walk`.
/// Then every file is parsed and filtered in `produce_item`.
/// Finally, `produce_item` will send `Item` to the consumer thread.
pub trait PathWorker: Worker {
  /// WalkParallel will determine what files will be processed.
  fn build_walk(&self) -> Result<WalkParallel>;
  /// Record trace for the worker.
  fn get_trace(&self) -> &FileTrace;
  /// Print why a file was skipped when entity inspection is enabled.
  fn print_skip(&self, path: &Path, err: &anyhow::Error) -> Result<()>;
  /// Parse and find_match can be done in `produce_item`.
  fn produce_item<P: Printer>(
    &self,
    path: &Path,
    processor: &P::Processor,
  ) -> Result<Vec<P::Processed>>;

  /// Returns true if the worker should stop processing files.
  /// Used to implement early termination (e.g., --max-results).
  fn should_stop(&self) -> bool {
    false
  }

  fn run_path<P: Printer>(self, printer: P) -> Result<ExitCode>
  where
    Self: Sized + 'static,
  {
    run_worker(Arc::new(self), printer)
  }
}

pub trait StdInWorker: Worker {
  fn parse_stdin<P: Printer>(
    &self,
    src: String,
    processor: &P::Processor,
  ) -> Result<Vec<P::Processed>>;

  fn run_std_in<P: Printer>(&self, printer: P) -> Result<ExitCode> {
    let source = std::io::read_to_string(std::io::stdin())?;
    let processor = printer.get_processor();
    if let Ok(items) = self.parse_stdin::<P>(source, &processor) {
      self.consume_items(Items::once(items)?, printer)
    } else {
      // return exit code 2 on parse error
      Ok(ExitCode::from(2))
    }
  }
}

pub struct Items<T>(mpsc::Receiver<T>);
impl<T> Iterator for Items<T> {
  type Item = T;
  fn next(&mut self) -> Option<Self::Item> {
    // TODO: add error reporting here
    self.0.recv().ok()
  }
}
impl<T> Items<T> {
  fn once(items: Vec<T>) -> Result<Self> {
    let (tx, rx) = mpsc::channel();
    for item in items {
      // use write to avoid send/sync trait bound
      match tx.send(item) {
        Ok(_) => (),
        Err(e) => return Err(anyhow!(e.to_string())),
      };
    }
    Ok(Items(rx))
  }
}

fn ignore_error_path(err: &ignore::Error) -> Option<&Path> {
  match err {
    ignore::Error::Partial(errors) => errors.iter().find_map(ignore_error_path),
    ignore::Error::WithLineNumber { err, .. } | ignore::Error::WithDepth { err, .. } => {
      ignore_error_path(err)
    }
    ignore::Error::WithPath { path, .. } => Some(path),
    ignore::Error::Loop { child, .. } => Some(child),
    _ => None,
  }
}

fn normalize_path(path: PathBuf) -> PathBuf {
  // TODO: is it correct here? see https://github.com/ast-grep/ast-grep/issues/1343
  match path.strip_prefix("./") {
    Ok(p) => p.to_path_buf(),
    Err(_) => path,
  }
}

fn filter_result(
  result: Result<DirEntry, ignore::Error>,
) -> Result<Option<PathBuf>, (PathBuf, anyhow::Error)> {
  let entry = match result {
    Ok(entry) => entry,
    Err(err) => {
      eprintln!("ERROR: {err}");
      let Some(path) = ignore_error_path(&err).map(Path::to_path_buf) else {
        return Ok(None);
      };
      return Err((normalize_path(path), err.into()));
    }
  };
  let Some(file_type) = entry.file_type() else {
    return Ok(None);
  };
  if !file_type.is_file() {
    return Ok(None);
  }
  Ok(Some(normalize_path(entry.into_path())))
}

fn run_worker<W: PathWorker + ?Sized + 'static, P: Printer>(
  worker: Arc<W>,
  printer: P,
) -> Result<ExitCode> {
  let (tx, rx) = mpsc::channel();
  let w = worker.clone();
  let walker = worker.build_walk()?;
  let processor = printer.get_processor();
  // walker run will block the thread
  std::thread::spawn(move || {
    let tx = tx;
    let processor = processor;
    walker.run(|| {
      let tx = tx.clone();
      let w = w.clone();
      let processor = &processor;
      Box::new(move |result| {
        let p = match filter_result(result) {
          Ok(Some(path)) => path,
          Ok(None) => return WalkState::Continue,
          Err((path, err)) => {
            let stats = w.get_trace();
            stats.add_scanned();
            stats.add_skipped();
            _ = w.print_skip(&path, &err);
            return WalkState::Continue;
          }
        };
        let stats = w.get_trace();
        stats.add_scanned();
        let items = match w.produce_item::<P>(&p, processor) {
          Ok(items) => items,
          Err(err) => {
            stats.add_skipped();
            _ = w.print_skip(&p, &err);
            return WalkState::Continue;
          }
        };
        for result in items {
          match tx.send(result) {
            Ok(_) => continue,
            Err(_) => return WalkState::Quit,
          }
        }
        if w.should_stop() {
          return WalkState::Quit;
        }
        WalkState::Continue
      })
    });
  });
  worker.consume_items(Items(rx), printer)
}

pub struct MaxItemCounter(AtomicUsize);

impl MaxItemCounter {
  /// The baseline is to pack two usize (max and curr item)
  /// into one atomic usize without underflowing
  pub const BASELINE: usize = 2usize << 20;

  pub fn new(max: u16) -> Self {
    Self(AtomicUsize::new(max as usize + Self::BASELINE))
  }

  /// returning the actual reserved count
  pub fn claim(&self, count: usize) -> usize {
    let count = count.min(Self::BASELINE);
    let prev = self.0.fetch_sub(count, Ordering::AcqRel);
    prev.saturating_sub(Self::BASELINE).min(count)
  }

  pub fn reached_max(&self) -> bool {
    self.0.load(Ordering::Acquire) <= Self::BASELINE
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ignore_error_path() {
    let error = ignore::Error::WithDepth {
      depth: 1,
      err: Box::new(ignore::Error::WithPath {
        path: "unreadable.js".into(),
        err: Box::new(ignore::Error::Io(std::io::Error::from(
          std::io::ErrorKind::PermissionDenied,
        ))),
      }),
    };
    assert_eq!(ignore_error_path(&error), Some(Path::new("unreadable.js")));
  }

  #[test]
  fn test_max_item_counter_initialization() {
    let counter = MaxItemCounter::new(100);
    assert_eq!(
      counter.0.load(Ordering::Acquire),
      100 + MaxItemCounter::BASELINE
    );
    assert!(!counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_claim_within_limit() {
    let counter = MaxItemCounter::new(10);

    // Claim 5 items, should get all 5
    let claimed = counter.claim(5);
    assert_eq!(claimed, 5);
    assert!(!counter.reached_max());

    // Claim 3 more, should get all 3
    let claimed = counter.claim(3);
    assert_eq!(claimed, 3);
    assert!(!counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_claim_exceeds_limit() {
    let counter = MaxItemCounter::new(5);

    // Claim 3 items
    let claimed = counter.claim(3);
    assert_eq!(claimed, 3);

    // Try to claim 5 more, but only 2 remain
    let claimed = counter.claim(5);
    assert_eq!(claimed, 2);
    assert!(counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_reached_max() {
    let counter = MaxItemCounter::new(3);

    assert!(!counter.reached_max());

    counter.claim(2);
    assert!(!counter.reached_max());

    counter.claim(1);
    assert!(counter.reached_max());

    // Additional claims should return 0
    let claimed = counter.claim(1);
    assert_eq!(claimed, 0);
    assert!(counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_claim_zero() {
    let counter = MaxItemCounter::new(10);

    let claimed = counter.claim(0);
    assert_eq!(claimed, 0);
    assert!(!counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_claim_more_than_baseline() {
    let counter = MaxItemCounter::new(10);

    // Try to claim more than BASELINE, should be clamped
    let huge_count = MaxItemCounter::BASELINE + 1000;
    let claimed = counter.claim(huge_count);

    // Should only claim up to the max (10)
    assert_eq!(claimed, 10);
    assert!(counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_multiple_small_claims() {
    let counter = MaxItemCounter::new(10);

    for _ in 0..10 {
      let claimed = counter.claim(1);
      assert_eq!(claimed, 1);
    }

    assert!(counter.reached_max());

    // Next claim should return 0
    let claimed = counter.claim(1);
    assert_eq!(claimed, 0);
  }

  #[test]
  fn test_max_item_counter_zero_max() {
    let counter = MaxItemCounter::new(0);

    assert!(counter.reached_max());

    let claimed = counter.claim(1);
    assert_eq!(claimed, 0);
  }

  #[test]
  fn test_max_item_counter_partial_claim() {
    let counter = MaxItemCounter::new(7);

    // Claim 10, but only 7 available
    let claimed = counter.claim(10);
    assert_eq!(claimed, 7);
    assert!(counter.reached_max());
  }

  #[test]
  fn test_max_item_counter_concurrent_claims() {
    use std::thread;

    let counter = Arc::new(MaxItemCounter::new(100));
    let mut handles = vec![];

    // Spawn 10 threads, each claiming 15 items
    for _ in 0..10 {
      let counter_clone = Arc::clone(&counter);
      let handle = thread::spawn(move || counter_clone.claim(15));
      handles.push(handle);
    }

    // Collect all claimed amounts
    let total_claimed: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

    // Total claimed should equal the max (100)
    assert_eq!(total_claimed, 100);
    assert!(counter.reached_max());
  }
}
