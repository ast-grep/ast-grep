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
    let is_light = terminal_light::luma().is_ok_and(|l| l > 0.6);
    if is_light {
      MadSkin::default_light()
    } else {
      MadSkin::default_dark()
    }
  }
}
