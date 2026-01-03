use crate::print::Printer;
use crate::utils::FileTrace;

use anyhow::{anyhow, Result};
use ignore::{DirEntry, WalkParallel, WalkState};

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{mpsc, Arc};

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
  /// Parse and find_match can be done in `produce_item`.
  fn produce_item<P: Printer>(
    &self,
    path: &Path,
    processor: &P::Processor,
  ) -> Result<Vec<P::Processed>>;

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
      Ok(ExitCode::FAILURE)
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

fn filter_result(entry: DirEntry,  stdin_path: & mut bool) -> Option<PathBuf> {
  let file = entry.file_type()?.is_file();
  let path = entry.into_path();
  *stdin_path = path.as_os_str() == "/dev/stdin";
  if !file {
    return None;
  }
  // TODO: is it correct here? see https://github.com/ast-grep/ast-grep/issues/1343
  match path.strip_prefix("./") {
    Ok(p) => Some(p.to_path_buf()),
    Err(_) => Some(path)
  }
}

fn scan_one<W: PathWorker + ?Sized + 'static, P: Printer>(tx: mpsc::Sender<<P as Printer>::Processed>, w: Arc<W>, processor: &<P as Printer>::Processor, p: PathBuf) -> WalkState {
    let stats = w.get_trace();
    stats.add_scanned();
    let Ok(items) = w.produce_item::<P>(&p, processor) else {
      stats.add_skipped();
      return WalkState::Continue;
    };
    for result in items {
      match tx.send(result) {
        Ok(_) => continue,
        Err(_) => return WalkState::Quit,
      }
    }
    return WalkState::Continue;
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
        let entry = match result {
          Ok(entry) => entry,
          Err(err) => {
            eprintln!("ERROR: {err}");
            return WalkState::Continue;
          }
        };
        let mut stdin_path: bool = false;
        if let Some(p) = filter_result(entry, &mut stdin_path) {
          return scan_one::<W, P>(tx.clone(), w.clone(), processor, p);
        } else if stdin_path {
          loop {
            let mut line = String::new();
            let path = match std::io::stdin().read_line(&mut line) {
              Ok(_) => line.trim_end(),
              Err(_) => break,
            };
            match path.is_empty() {
              true => return WalkState::Continue,
              false => scan_one::<W, P>(tx.clone(), w.clone(), processor, PathBuf::from(path)),
            };
          }
        }
        return WalkState::Continue;
      })
    });
  });
  worker.consume_items(Items(rx), printer)
}
