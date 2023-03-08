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
  Ok(())
}

fn is_cwd_under_a_project() -> bool {
  todo!()
}

fn run_create_entity(entity: Entity) {
  // check if we creating a project
  if entity == Entity::Project {
    // create the project if user choose to create
    println!("creating project!");
    todo!()
  }
  // check if we are under a project dir
  if !is_cwd_under_a_project() {
    // if not, return error
    return;
  }
  // ask user what destination to create if multiple dirs exist
  println!("{entity:?}");
  // ask if a test is needed if user is creating a rule
  if entity == Entity::Rule {
    println!("Do you also need to create a rule?");
    todo!()
  }
}

// TODO:
// 1. check if we are under a sgconfig.yml
// 2. ask users what to create if yes
// 3. ask users to provide project info if no sgconfig found
fn ask_entity_type() {
  if is_cwd_under_a_project() {
    println!("What do you want to create?");
  } else {
    println!("creating project!");
  }
}
