use ast_grep_language::SupportLang;

mod common;

const JAVASCRIPT_RULES: &str = include_str!("../src/default_rules/javascript.yml");

#[test]
fn extracts_javascript_outline_from_es_module_code() {
  common::assert_outline_snapshot(
    SupportLang::JavaScript,
    JAVASCRIPT_RULES,
    r#"
import fs from 'fs';
import { join as pathJoin } from 'path';

export { localHelper };
export { readFile as read } from './io.js';
export * from './all.js';

const LOCAL = 1;
export const EXPORTED = 2;
let state;
let count = 0;
const makeThing = () => ({});

function localHelper() {}

export function run() {}

setup(() => {
  const callbackLocal = () => {};
  function nestedHelper() {}
});

class LocalBox {
  static version = 1;
  #secret = 2;

  constructor(name) {
    this.name = name;
  }

  get value() {
    return this.#secret;
  }

  hide() {}
  #reset() {}
}

export class Service extends Base {
  start() {}
}
"#,
    r#"
- Module import private 'fs'
- Module import private 'path'
- Module item exported exports
- Module item exported './io.js'
- Module item exported './all.js'
- Constant item private LOCAL
- Constant item exported EXPORTED
- Variable item private state
- Variable item private count
- Function item private makeThing
- Function item private localHelper
- Function item exported run
- Class item private LocalBox
  - Field public version
  - Field private #secret
  - Constructor public constructor
  - Method public value
  - Method public hide
  - Method private #reset
- Class item exported Service
  - Method public start
"#,
  );
}

#[test]
fn extracts_javascript_signatures() {
  common::assert_outline_signature_snapshot(
    SupportLang::JavaScript,
    JAVASCRIPT_RULES,
    r#"
export class Service {
  constructor(name) {
    this.name = name;
  }

  start() {}
  #reset() {}
}
"#,
    r#"
- Class item exported Service | export class Service {
  - Constructor public constructor | constructor(name) {
  - Method public start | start() {}
  - Method private #reset | #reset() {}
"#,
  );
}
