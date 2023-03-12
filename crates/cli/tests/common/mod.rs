use std::fs::File;
use std::io::Write;
use tempdir::TempDir;

pub fn create_test_files<'a>(
  names_and_contents: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> TempDir {
  let dir = TempDir::new("sgtest").unwrap();
  for (name, contents) in names_and_contents {
    let path = dir.path().join(name);
    let mut file = File::create(path.clone()).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file.sync_all().unwrap();
  }
  dir
}
