use crate::languages::{config_file_type, SupportLang};
use ast_grep_config::{from_yaml_string, Configs};
use ignore::WalkBuilder;
use std::fs::read_to_string;

pub fn find_config(config: Option<String>) -> Configs<SupportLang> {
    let config_file_or_dir = config.unwrap_or_else(find_default_config);
    let mut configs = vec![];
    let walker = WalkBuilder::new(&config_file_or_dir)
        .types(config_file_type())
        .build();
    for dir in walker {
        let config_file = dir.unwrap();
        if !config_file.file_type().unwrap().is_file() {
            continue;
        }
        let path = config_file.path();

        let yaml = read_to_string(path).unwrap();
        configs.extend(from_yaml_string(&yaml).unwrap());
    }
    Configs::new(configs)
}

fn find_default_config() -> String {
    "sgconfig.yml".to_string()
}
