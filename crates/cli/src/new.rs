use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct NewArg {
  #[clap(subcommand)]
  entity: Option<Entity>,
}

#[derive(Subcommand, Debug)]
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

fn run_create_entity(entity: Entity) {
  println!("{entity:?}")
}

fn ask_entity_type() {
  println!("What do you want to create?")
}
