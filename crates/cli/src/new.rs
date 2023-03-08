use crate::config::read_sg_config_from_current_dir;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct NewArg {
  #[clap(subcommand)]
  entity: Option<Entity>,
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

pub fn run_create_new(arg: NewArg) -> Result<()> {
  if let Some(entity) = arg.entity {
    run_create_entity(entity)
  } else {
    ask_entity_type()
  }
}

fn run_create_entity(entity: Entity) -> Result<()> {
  // check if we creating a project
  if entity == Entity::Project {
    // create the project if user choose to create
    create_new_project();
    return Ok(());
  }
  // check if we are under a project dir
  let Some(sg_config) = read_sg_config_from_current_dir()? else {
    // if not, return error
    return Err(anyhow::anyhow!("TODO: add proper error message"));
  };
  // ask user what destination to create if multiple dirs exist
  println!("{entity:?}");
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
fn ask_entity_type() -> Result<()> {
  if let Some(sg_config) = read_sg_config_from_current_dir()? {
    println!("What do you want to create?");
  } else {
    create_new_project();
  }
  Ok(())
}

fn create_new_project() {
  println!("creating project!");
}

fn create_new_test() {
  println!("create test!");
}
