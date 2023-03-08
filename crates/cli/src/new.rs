use crate::config::{read_sg_config_from_current_dir, AstGrepConfig, TestConfig};

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_yaml::to_writer;

use std::fmt::Display;
use std::fs::File;
use std::path::PathBuf;

#[derive(Parser)]
pub struct NewArg {
  /// TODO: add doc
  #[clap(subcommand)]
  entity: Option<Entity>,
  /// TODO: add doc
  #[clap(short, long)]
  accept_all: bool,
  /// TODO: add doc
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
  // check if we creating a project
  if entity == Entity::Project {
    // create the project if user choose to create
    return create_new_project();
  }
  // check if we are under a project dir
  let Some(sg_config) = read_sg_config_from_current_dir()? else {
    // if not, return error
    return Err(anyhow::anyhow!("TODO: add proper error message"));
  };
  do_create_entity(entity, sg_config, arg)
}

fn do_create_entity(entity: Entity, sg_config: AstGrepConfig, arg: NewArg) -> Result<()> {
  // ask user what destination to create if multiple dirs exist
  println!("Creating {entity}");
  // ask if a test is needed if user is creating a rule
  if entity == Entity::Rule {
    println!("Do you also need to create a rule?");
    create_new_test();
  }
  Ok(())
}

// TODO:
// 1. check if we are under a sgconfig.yml
// 2. ask users what to create if yes
// 3. ask users to provide project info if no sgconfig found
fn ask_entity_type(arg: NewArg) -> Result<()> {
  if let Some(sg_config) = read_sg_config_from_current_dir()? {
    let entity = inquire::Select::new(
      "Select the item you want to create:",
      vec![Entity::Rule, Entity::Test, Entity::Util],
    )
    .prompt()?;
    do_create_entity(entity, sg_config, arg)
  } else {
    create_new_project()
  }
}

fn create_new_project() -> Result<()> {
  println!("Creating a new ast-grep project...");
  let rule_dirs = inquire::Text::new("Where do you want to have your rules?")
    .with_default("rules")
    .prompt()?;
  let test_dirs = if inquire::Confirm::new("Do you want to create rule tests?")
    .with_default(true)
    .prompt()?
  {
    let test = inquire::Text::new("Where do you want to have your tests?")
      .with_default("rule-test")
      .prompt()?;
    Some(TestConfig::from(test))
  } else {
    None
  };
  let utils = if inquire::Confirm::new("Do you want to create folder for utility rules?")
    .with_default(true)
    .prompt()?
  {
    Some(
      inquire::Text::new("Where do you want to have your tests?")
        .with_default("rule-test")
        .prompt()?,
    )
  } else {
    None
  };
  let root_config = AstGrepConfig {
    rule_dirs: vec![PathBuf::from(rule_dirs)],
    test_configs: test_dirs.map(|t| vec![t]),
    util_dirs: utils.map(|u| vec![PathBuf::from(u)]),
  };
  let f = File::create("sgconfig.yml")?;
  to_writer(f, &root_config)?;
  Ok(())
}

fn create_new_test() {
  println!("create test!");
}
