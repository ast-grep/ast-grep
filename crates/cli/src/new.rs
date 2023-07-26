use crate::config::{read_config_from_dir, register_custom_language, AstGrepConfig, TestConfig};
use crate::error::ErrorContext as EC;
use crate::lang::SgLang;

use anyhow::Result;
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
  #[arg(value_parser, global = true)]
  name: Option<String>,
  /// The language of the item. Appliable to rule and utils.
  #[arg(short, long, global = true)]
  lang: Option<SgLang>,
  /// Accept all default options without interactive input during creation.
  #[arg(short, long, global = true)]
  yes: bool,
  #[arg(short, long, global = true, default_value = ".")]
  base_dir: PathBuf,
}

impl NewArg {
  fn ask_dir_and_create(&self, prompt: &str, default: &str) -> Result<PathBuf> {
    let dir = if self.yes {
      default.to_owned()
    } else {
      inquire::Text::new(prompt).with_default(default).prompt()?
    };
    let path = self.base_dir.join(dir);
    fs::create_dir_all(&path)?;
    Ok(path)
  }

  fn confirm(&self, prompt: &str) -> Result<bool> {
    if self.yes {
      return Ok(true);
    }
    Ok(inquire::Confirm::new(prompt).with_default(true).prompt()?)
  }

  fn ask_entity_type(&self) -> Result<Entity> {
    if self.yes {
      self
        .entity
        .clone()
        .map(Ok)
        .unwrap_or_else(|| Err(anyhow::anyhow!(EC::InsufficientCLIArgument("entity"))))
    } else {
      let entity = inquire::Select::new(
        "Select the item you want to create:",
        vec![Entity::Rule, Entity::Test, Entity::Util],
      )
      .prompt()?;
      Ok(entity)
    }
  }

  fn choose_language(&self) -> Result<SgLang> {
    if let Some(lang) = self.lang {
      Ok(lang)
    } else if self.yes {
      Err(anyhow::anyhow!(EC::InsufficientCLIArgument("lang")))
    } else {
      Ok(inquire::Select::new("Choose rule's language", SgLang::all_langs()).prompt()?)
    }
  }

  fn ask_name(&self, entity: &'static str) -> Result<String> {
    if let Some(name) = &self.name {
      Ok(name.to_string())
    } else if self.yes {
      Err(anyhow::anyhow!(EC::InsufficientCLIArgument("name")))
    } else {
      Ok(
        inquire::Text::new(&format!("What is your {entity}'s name?"))
          .with_validator(ValueRequiredValidator::default())
          .prompt()?,
      )
    }
  }
}

#[derive(Subcommand, Debug, PartialEq, Eq, Clone)]
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
  register_custom_language(Some(arg.base_dir.clone()));
  if let Some(entity) = arg.entity.take() {
    run_create_entity(entity, arg)
  } else {
    ask_entity_type(arg)
  }
}

// base_dir, config
type FoundConfig = (PathBuf, AstGrepConfig);

fn run_create_entity(entity: Entity, arg: NewArg) -> Result<()> {
  // check if we are under a project dir
  if let Some(found) = read_config_from_dir(&arg.base_dir)? {
    return do_create_entity(entity, found, arg);
  }
  // check if we creating a project
  if entity == Entity::Project {
    create_new_project(arg)
  } else {
    // if not, return error
    Err(anyhow::anyhow!(EC::ProjectNotExist))
  }
}

fn do_create_entity(entity: Entity, found: FoundConfig, arg: NewArg) -> Result<()> {
  // ask user what destination to create if multiple dirs exist
  match entity {
    Entity::Rule => create_new_rule(found, arg),
    Entity::Test => create_new_test(found.1.test_configs, arg.name),
    Entity::Util => create_new_util(found, arg),
    Entity::Project => Err(anyhow::anyhow!(EC::ProjectAlreadyExist)),
  }
}

fn ask_entity_type(arg: NewArg) -> Result<()> {
  // 1. check if we are under a sgconfig.yml
  if let Some(found) = read_config_from_dir(&arg.base_dir)? {
    // 2. ask users what to create if yes
    let entity = arg.ask_entity_type()?;
    do_create_entity(entity, found, arg)
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
    let test_dirs =
      arg.ask_dir_and_create("Where do you want to have your tests?", "rule-tests")?;
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
    custom_languages: None, // advanced feature, skip now
  };
  let config_path = arg.base_dir.join("sgconfig.yml");
  let f = File::create(config_path)?;
  serde_yaml::to_writer(f, &root_config)?;
  println!("Your new ast-grep project has been created!");
  Ok(())
}

fn default_rule(id: &str, lang: SgLang) -> String {
  format!(
    r#"id: {id}
message: Add your rule message here....
severity: error # error, warning, info, hint
language: {lang}
rule:
  pattern: Your Rule Pattern here...
# utils: Extract repeated rule as local utility here.
# note: Add detailed explanation for the rule."#
  )
}

fn create_new_rule(found: FoundConfig, arg: NewArg) -> Result<()> {
  let (base_dir, sg_config) = found;
  let name = arg.ask_name("rule")?;
  let rule_dir = if sg_config.rule_dirs.len() > 1 {
    let dirs = sg_config.rule_dirs.iter().map(|p| p.display()).collect();
    let display =
      inquire::Select::new("Which rule dir do you want to save your rule?", dirs).prompt()?;
    base_dir.join(display.to_string())
  } else {
    base_dir.join(&sg_config.rule_dirs[0])
  };
  let path = rule_dir.join(format!("{name}.yml"));
  if path.exists() {
    return Err(anyhow::anyhow!(EC::FileAlreadyExist(path)));
  }
  let lang = arg.choose_language()?;
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

fn default_util(id: &str, lang: SgLang) -> String {
  format!(
    r#"id: {id}
language: {lang}
rule:
  pattern: Your Rule Pattern here...
# utils: Extract repeated rule as local utility here."#
  )
}

fn create_new_util(found: FoundConfig, arg: NewArg) -> Result<()> {
  let (base_dir, sg_config) = found;
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
    base_dir.join(display.to_string())
  } else {
    base_dir.join(&utils[0])
  };
  let name = arg.ask_name("util")?;
  let path = util_dir.join(format!("{name}.yml"));
  if path.exists() {
    return Err(anyhow::anyhow!(EC::FileAlreadyExist(path)));
  }
  let lang = arg.choose_language()?;
  fs::write(&path, default_util(&name, lang))?;
  println!("Created util at {}", path.display());
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_language::SupportLang;
  use std::path::Path;
  use tempdir::TempDir;

  fn create_project(tempdir: &Path) -> Result<()> {
    let arg = NewArg {
      entity: None,
      name: None,
      lang: None,
      yes: true,
      base_dir: tempdir.to_path_buf(),
    };
    run_create_new(arg)?;
    assert!(tempdir.join("sgconfig.yml").exists());
    Ok(())
  }

  fn create_rule(temp: &Path) -> Result<()> {
    let arg = NewArg {
      entity: Some(Entity::Rule),
      name: Some("test-rule".into()),
      lang: Some(SupportLang::Rust.into()),
      yes: true,
      base_dir: temp.to_path_buf(),
    };
    run_create_new(arg).unwrap();
    assert!(temp.join("rules/test-rule.yml").exists());
    Ok(())
  }

  fn create_util(temp: &Path) -> Result<()> {
    let arg = NewArg {
      entity: Some(Entity::Util),
      name: Some("test-utils".into()),
      lang: Some(SupportLang::Rust.into()),
      yes: true,
      base_dir: temp.to_path_buf(),
    };
    run_create_new(arg).unwrap();
    assert!(temp.join("utils/test-utils.yml").exists());
    Ok(())
  }

  #[test]
  fn test_create_new() -> Result<()> {
    let dir = TempDir::new("sgtest")?;
    create_project(dir.path())?;
    create_rule(dir.path())?;
    drop(dir); // drop at the end since temp dir clean up is done in Drop
    Ok(())
  }

  #[test]
  fn test_create_util() -> Result<()> {
    let dir = TempDir::new("sgtest")?;
    create_project(dir.path())?;
    create_util(dir.path())?;
    drop(dir); // drop at the end since temp dir clean up is done in Drop
    Ok(())
  }
}
