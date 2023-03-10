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
  let maybe_sg_config = read_sg_config_from_current_dir()?;
  // check if we creating a project
  if entity == Entity::Project {
    return if maybe_sg_config.is_some() {
      Err(anyhow::anyhow!(EC::ProjectAlreadyExist))
    } else {
      // create the project if user choose to create
      create_new_project()
    };
  }
  // check if we are under a project dir
  let Some(sg_config) = maybe_sg_config else {
    // if not, return error
    return Err(anyhow::anyhow!(EC::ProjectNotExist));
  };
  do_create_entity(entity, sg_config, arg)
}

fn do_create_entity(entity: Entity, sg_config: AstGrepConfig, arg: NewArg) -> Result<()> {
  // ask user what destination to create if multiple dirs exist
  match entity {
    Entity::Rule => create_new_rule(sg_config, arg)?,
    Entity::Test => create_new_test(sg_config.test_configs, arg.name)?,
    Entity::Util => create_new_util(sg_config, arg)?,
    _ => unreachable!(),
  }
  Ok(())
}

fn ask_dir_and_create(prompt: &str, default: &str) -> Result<PathBuf> {
  let dir = inquire::Text::new(prompt).with_default(default).prompt()?;
  let path = PathBuf::from(dir);
  fs::create_dir_all(&path)?;
  Ok(path)
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
    create_new_project()
  }
}

fn create_new_project() -> Result<()> {
  println!("Creating a new ast-grep project...");
  let rule_dirs = ask_dir_and_create("Where do you want to have your rules?", "rules")?;
  let test_dirs = if inquire::Confirm::new("Do you want to create rule tests?")
    .with_default(true)
    .prompt()?
  {
    let test_dirs = ask_dir_and_create("Where do you want to have your tests?", "rule-test")?;
    Some(TestConfig::from(test_dirs))
  } else {
    None
  };
  let utils = if inquire::Confirm::new("Do you want to create folder for utility rules?")
    .with_default(true)
    .prompt()?
  {
    let util_dirs = ask_dir_and_create("Where do you want to have your utilities?", "utils")?;
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
  let name = if let Some(name) = arg.name {
    name
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
  let need_test = inquire::Confirm::new("Do you also need to create a test for the rule?")
    .with_default(true)
    .prompt()?;
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
