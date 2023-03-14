use crate::config::{read_sg_config_from_current_dir, AstGrepConfig, TestConfig};
use crate::error::ErrorContext as EC;

use anyhow::Result;
use ast_grep_language::SupportLang;
use clap::{Parser, Subcommand};
use inquire::validator::ValueRequiredValidator;

use std::fmt::Display;
use std::fs::{self, File};
use std::path::PathBuf;

#[derive(Parser)]
pub struct NewArg {
  /// The ast-grep item type to create. Available options: project/rule/test/utils.
  #[clap(subcommand)]
  entity: Option<Entity>,
  /// The id of the item to create.
  #[clap(value_parser)]
  name: Option<String>,
  /// The language of the item. Appliable to rule and utils.
  #[clap(short, long)]
  lang: Option<SupportLang>,
  /// Accept all default options without interactive input during creation.
  #[clap(short, long)]
  yes: bool,
}

impl NewArg {
  fn ask_dir_and_create(&self, prompt: &str, default: &str) -> Result<PathBuf> {
    let dir = if self.yes {
      default.to_owned()
    } else {
      inquire::Text::new(prompt).with_default(default).prompt()?
    };
    let path = PathBuf::from(dir);
    fs::create_dir_all(&path)?;
    Ok(path)
  }

  fn confirm(&self, prompt: &str) -> Result<bool> {
    if self.yes {
      return Ok(true);
    }
    Ok(inquire::Confirm::new(prompt).with_default(true).prompt()?)
  }
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum Entity {
  Project,
  Rule,
  Test,
  Util,
}

impl Display for Entity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    use Entity::*;
    match self {
      Project => f.write_str("Project"),
      Rule => f.write_str("Rule"),
      Test => f.write_str("Test"),
      Util => f.write_str("Util"),
    }
  }
}

pub fn run_create_new(mut arg: NewArg) -> Result<()> {
  if let Some(entity) = arg.entity.take() {
    run_create_entity(entity, arg)
  } else {
    ask_entity_type(arg)
  }
}

fn run_create_entity(entity: Entity, arg: NewArg) -> Result<()> {
  // check if we are under a project dir
  if let Some(sg_config) = read_sg_config_from_current_dir()? {
    return do_create_entity(entity, sg_config, arg);
  }
  // check if we creating a project
  if entity == Entity::Project {
    create_new_project(arg)
  } else {
    // if not, return error
    Err(anyhow::anyhow!(EC::ProjectNotExist))
  }
}

fn do_create_entity(entity: Entity, sg_config: AstGrepConfig, arg: NewArg) -> Result<()> {
  // ask user what destination to create if multiple dirs exist
  match entity {
    Entity::Rule => create_new_rule(sg_config, arg),
    Entity::Test => create_new_test(sg_config.test_configs, arg.name),
    Entity::Util => create_new_util(sg_config, arg),
    Entity::Project => Err(anyhow::anyhow!(EC::ProjectAlreadyExist)),
  }
}

fn ask_entity_type(arg: NewArg) -> Result<()> {
  // 1. check if we are under a sgconfig.yml
  if let Some(sg_config) = read_sg_config_from_current_dir()? {
    // 2. ask users what to create if yes
    let entity = inquire::Select::new(
      "Select the item you want to create:",
      vec![Entity::Rule, Entity::Test, Entity::Util],
    )
    .prompt()?;
    do_create_entity(entity, sg_config, arg)
  } else {
    // 3. ask users to provide project info if no sgconfig found
    print!("No sgconfig.yml found. ");
    create_new_project(arg)
  }
}

fn create_new_project(arg: NewArg) -> Result<()> {
  println!("Creating a new ast-grep project...");
  let rule_dirs = arg.ask_dir_and_create("Where do you want to have your rules?", "rules")?;
  let test_dirs = if arg.confirm("Do you want to create rule tests?")? {
    let test_dirs = arg.ask_dir_and_create("Where do you want to have your tests?", "rule-test")?;
    Some(TestConfig::from(test_dirs))
  } else {
    None
  };
  let utils = if arg.confirm("Do you want to create folder for utility rules?")? {
    let util_dirs = arg.ask_dir_and_create("Where do you want to have your utilities?", "utils")?;
    Some(util_dirs)
  } else {
    None
  };
  let root_config = AstGrepConfig {
    rule_dirs: vec![rule_dirs],
    test_configs: test_dirs.map(|t| vec![t]),
    util_dirs: utils.map(|u| vec![u]),
  };
  let f = File::create("sgconfig.yml")?;
  serde_yaml::to_writer(f, &root_config)?;
  println!("Your new ast-grep project is created!");
  Ok(())
}

fn choose_language() -> Result<SupportLang> {
  Ok(inquire::Select::new("Choose rule's language", SupportLang::all_langs()).prompt()?)
}

fn default_rule(id: &str, lang: SupportLang) -> String {
  format!(
    r#"id: {id}
message: Add your rule message here....
severity: error # error, warning, hint, info
language: {lang}
rule:
  pattern: Your Rule Pattern here...
# utils: Extract repeated rule as local utility here.
# note: Add detailed explanation for the rule."#
  )
}

fn create_new_rule(sg_config: AstGrepConfig, arg: NewArg) -> Result<()> {
  let name = if let Some(name) = &arg.name {
    name.to_string()
  } else {
    inquire::Text::new("What is your rule name?")
      .with_validator(ValueRequiredValidator::default())
      .prompt()?
  };
  let rule_dir = if sg_config.rule_dirs.len() > 1 {
    let dirs = sg_config.rule_dirs.iter().map(|p| p.display()).collect();
    let display =
      inquire::Select::new("Which rule dir do you want to save your rule?", dirs).prompt()?;
    PathBuf::from(display.to_string())
  } else {
    sg_config.rule_dirs[0].clone()
  };
  let path = rule_dir.join(format!("{name}.yml"));
  if path.exists() {
    return Err(anyhow::anyhow!(EC::FileAlreadyExist(path)));
  }
  let lang = choose_language()?;
  fs::write(&path, default_rule(&name, lang))?;
  println!("Created rules at {}", path.display());
  let need_test = arg.confirm("Do you also need to create a test for the rule?")?;
  if need_test {
    create_new_test(sg_config.test_configs, Some(name))?;
  }
  Ok(())
}

fn default_test(id: &str) -> String {
  format!(
    r#"id: {id}
valid:
- "valid code"
invalid:
- "invalid code"
"#
  )
}

fn create_new_test(test_configs: Option<Vec<TestConfig>>, name: Option<String>) -> Result<()> {
  let Some(tests) = test_configs else {
    return Err(anyhow::anyhow!(EC::NoTestDirConfigured))
  };
  if tests.is_empty() {
    return Err(anyhow::anyhow!(EC::NoTestDirConfigured));
  }
  let test_dir = if tests.len() > 1 {
    let dirs = tests.iter().map(|t| t.test_dir.display()).collect();
    let display = inquire::Select::new("Which test dir do you want to use?", dirs).prompt()?;
    PathBuf::from(display.to_string())
  } else {
    tests[0].test_dir.clone()
  };
  let name = if let Some(name) = name {
    name
  } else {
    inquire::Text::new("What is the rule's id that you want to test?")
      .with_validator(ValueRequiredValidator::default())
      .prompt()?
  };
  let path = test_dir.join(format!("{name}-test.yml"));
  if path.exists() {
    return Err(anyhow::anyhow!(EC::FileAlreadyExist(path)));
  }
  fs::write(&path, default_test(&name))?;
  println!("Created test at {}", path.display());
  Ok(())
}

fn default_util(id: &str, lang: SupportLang) -> String {
  format!(
    r#"id: {id}
language: {lang}
rule:
  pattern: Your Rule Pattern here...
# utils: Extract repeated rule as local utility here."#
  )
}

fn create_new_util(sg_config: AstGrepConfig, arg: NewArg) -> Result<()> {
  let Some(utils) = sg_config.util_dirs else {
    return Err(anyhow::anyhow!(EC::NoUtilDirConfigured));
  };
  if utils.is_empty() {
    return Err(anyhow::anyhow!(EC::NoUtilDirConfigured));
  }
  let util_dir = if utils.len() > 1 {
    let dirs = utils.iter().map(|p| p.display()).collect();
    let display =
      inquire::Select::new("Which util dir do you want to save your rule?", dirs).prompt()?;
    PathBuf::from(display.to_string())
  } else {
    utils[0].clone()
  };
  let name = if let Some(name) = arg.name {
    name
  } else {
    inquire::Text::new("What is your util name?")
      .with_validator(ValueRequiredValidator::default())
      .prompt()?
  };
  let path = util_dir.join(format!("{name}.yml"));
  if path.exists() {
    return Err(anyhow::anyhow!(EC::FileAlreadyExist(path)));
  }
  let lang = choose_language()?;
  fs::write(&path, default_util(&name, lang))?;
  println!("Created util at {}", path.display());
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use tempdir::TempDir;

  #[test]
  fn test_create_new_project() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let dir = TempDir::new("sgtest")?;
    std::env::set_current_dir(&dir)?;
    let arg = NewArg {
      entity: None,
      name: None,
      lang: None,
      yes: true,
    };
    run_create_new(arg)?;
    assert!(PathBuf::from("sgconfig.yml").exists());
    std::env::set_current_dir(current_dir)?;
    drop(dir); // drop at the end since temp dir clean up is done in Drop
    Ok(())
  }
}
