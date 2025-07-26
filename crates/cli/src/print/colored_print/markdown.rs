use crate::lang::SgLang;
use ast_grep_config::RuleConfig;
use dashmap::DashMap;
use termimad::MadSkin;

/// A Markdown renderer that caches rendered notes to avoid recomputing.
pub struct Markdown {
  cache: DashMap<String, String>,
  skin: MadSkin,
}

impl Markdown {
  pub fn new(color: bool) -> Self {
    Self {
      cache: DashMap::new(),
      skin: Self::skin(color),
    }
  }

  pub fn render_note(&self, rule: &RuleConfig<SgLang>) -> Option<String> {
    let note = rule.note.as_ref()?;
    if let Some(cached) = self.cache.get(&rule.id) {
      return Some(cached.clone());
    }
    let rendered = self.skin.text(note, None).to_string();
    self.cache.insert(rule.id.clone(), rendered.clone());
    Some(rendered)
  }

  fn skin(color: bool) -> MadSkin {
    if !color {
      return MadSkin::no_style();
    }
    let is_light = is_light_terminal();
    if is_light {
      MadSkin::default_light()
    } else {
      MadSkin::default_dark()
    }
  }
}

fn is_light_terminal() -> bool {
  use terminal_light as tl;
  // prefer using env instead of escape sequences
  // https://github.com/ast-grep/ast-grep/issues/2114
  if let Ok(color) = tl::env::bg_color() {
    tl::Color::Ansi(color).luma() > 0.6
  } else {
    tl::luma().is_ok_and(|l| l > 0.6)
  }
}
