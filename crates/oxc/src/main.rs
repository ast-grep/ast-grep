mod binding;
use ast_grep_core::{AstGrep, Pattern};
use binding::{OxcDoc, OxcLang};
use oxc_span::SourceType;

fn main() -> std::io::Result<()> {
  let args = std::env::args().collect::<Vec<_>>();
  let name = args.get(1).expect("Must provide a file");
  let pat = args.get(2).expect("Must provide a pattern");
  let path = std::path::Path::new(&name);
  let source_text = std::fs::read_to_string(path)?;
  let lang = OxcLang(SourceType::from_path(path).unwrap());
  let doc = OxcDoc::try_new(source_text, lang).expect("Failed to parse");
  let sg = AstGrep::doc(doc);
  let pattern = Pattern::new(pat, lang);
  for m in sg.root().find_all(&pattern) {
    println!("Oxc ast-grep Found: {}", m.text());
  }
  Ok(())
}
