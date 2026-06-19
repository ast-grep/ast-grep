use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_language::SupportLang;
use ast_grep_outline::{
  combined_extractor::CombinedExtractors,
  extractor::{SerializableOutlineRule, parse_outline_rules},
  model::SymbolType,
};

const TYPESCRIPT_RULES: &str = include_str!("../src/default_rules/typescript.yml");

fn rules_for(lang: SupportLang) -> Vec<SerializableOutlineRule<SupportLang>> {
  parse_outline_rules::<SupportLang>(TYPESCRIPT_RULES)
    .expect("TypeScript outline rules should deserialize")
    .into_iter()
    .filter(|rule| rule.common().language == lang)
    .collect()
}

fn compile_for(lang: SupportLang) -> CombinedExtractors<SupportLang> {
  CombinedExtractors::try_from(rules_for(lang), &Default::default())
    .expect("TypeScript outline rules should compile")
}

#[test]
fn bundled_typescript_and_tsx_rules_compile() {
  compile_for(SupportLang::TypeScript);
  compile_for(SupportLang::Tsx);
}

#[test]
fn extracts_typescript_outline_from_realistic_vscode_style_code() {
  let combined = compile_for(SupportLang::TypeScript);
  let grep = SupportLang::TypeScript.ast_grep(
    r#"
import * as DOM from '../../../../../../base/browser/dom.js';
import { Lazy } from '../../../../../../base/common/lazy.js';
import type { INotebookEditorContribution } from '../../notebookBrowser.js';

export { NotebookFindFilters };
export { INotebookFindScope } from '../../../common/notebookCommon.js';

const FIND_SHOW_TRANSITION = 'find-show-transition';
export const FIND_HIDE_TRANSITION = 'find-hide-transition';
export let MAX_MATCHES_COUNT_WIDTH: number = 69;
let validateIndicesThrottled: ReturnType<typeof throttleRAF>;
const getNonDeletedElements = (elements: readonly ExcalidrawElement[]) => {
  return elements.filter((element) => !element.isDeleted);
};
const hashSelectionOpts = ({ selectedElementIds }: { selectedElementIds: Readonly<Record<string, true>> }) => {
  return Object.keys(selectedElementIds).join(',');
};
const noop = () => {}, _RPCProtocolSymbol = Symbol.for('rpc.protocol');

export interface IShowNotebookFindWidgetOptions {
  focus?: boolean;
  findScope?: INotebookFindScope;
  matchIndex?: number;
}

export type FindDirection = 'next' | 'previous';

export enum MatchKind {
  Exact,
  Fuzzy = 'fuzzy',
}

export function createFindWidget(): NotebookFindWidget {
  return new NotebookFindWidget();
}

function localHelper() {}

setup(() => {
  const callbackLocal = () => {};
  let callbackState = 1;
});

export class NotebookFindContrib extends Disposable implements INotebookEditorContribution {
  static readonly id: string = 'workbench.notebook.find';
  private readonly _widget: Lazy<NotebookFindWidget>;

  constructor(private readonly notebookEditor: INotebookEditor) {
    super();
  }

  get widget(): NotebookFindWidget {
    return this._widget.value;
  }

  hide() {
    this._widget.rawValue?.hide();
  }

  private _reset(): void {}
}

class NotebookFindWidget extends SimpleFindReplaceWidget {
  protected _findWidgetVisible: boolean = false;

  show(initialInput?: string): Promise<void> {
    return Promise.resolve();
  }
}
"#,
  );

  let items = combined.extract(grep.root());
  let names = items
    .iter()
    .map(|item| item.entry.name.as_ref())
    .collect::<Vec<_>>();

  assert!(items.iter().any(|item| {
    item.is_import
      && item
        .entry
        .name
        .contains("../../../../../../base/browser/dom.js")
  }));
  assert!(
    items
      .iter()
      .any(|item| { item.is_import && item.entry.name.contains("../../notebookBrowser.js") })
  );
  assert!(items.iter().any(|item| {
    item.is_exported
      && item
        .entry
        .name
        .contains("../../../common/notebookCommon.js")
  }));
  assert!(names.contains(&"FIND_HIDE_TRANSITION"));
  assert!(names.contains(&"MAX_MATCHES_COUNT_WIDTH"));
  assert!(names.contains(&"IShowNotebookFindWidgetOptions"));
  assert!(names.contains(&"FindDirection"));
  assert!(names.contains(&"MatchKind"));
  assert!(names.contains(&"createFindWidget"));
  assert!(names.contains(&"localHelper"));
  assert!(names.contains(&"NotebookFindContrib"));
  assert!(names.contains(&"NotebookFindWidget"));
  assert!(names.contains(&"FIND_SHOW_TRANSITION"));
  assert!(names.contains(&"validateIndicesThrottled"));
  assert!(names.contains(&"getNonDeletedElements"));
  assert!(names.contains(&"hashSelectionOpts"));
  assert!(names.contains(&"noop"));
  assert!(names.contains(&"_RPCProtocolSymbol"));
  assert!(!names.contains(&"callbackLocal"));
  assert!(!names.contains(&"callbackState"));

  let top_level_symbols = items
    .iter()
    .map(|item| {
      (
        item.entry.name.as_ref(),
        item.entry.symbol_type,
        item.is_exported,
      )
    })
    .collect::<Vec<_>>();
  assert!(top_level_symbols.contains(&("FIND_SHOW_TRANSITION", SymbolType::Constant, false)));
  assert!(top_level_symbols.contains(&("validateIndicesThrottled", SymbolType::Variable, false)));
  assert!(top_level_symbols.contains(&("getNonDeletedElements", SymbolType::Function, false)));
  assert!(top_level_symbols.contains(&("hashSelectionOpts", SymbolType::Function, false)));
  assert!(top_level_symbols.contains(&("noop", SymbolType::Function, false)));
  assert!(top_level_symbols.contains(&("_RPCProtocolSymbol", SymbolType::Constant, false)));

  let options = items
    .iter()
    .find(|item| item.entry.name == "IShowNotebookFindWidgetOptions")
    .expect("interface should be extracted");
  let option_members = options
    .members
    .iter()
    .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
    .collect::<Vec<_>>();
  assert_eq!(
    option_members,
    vec![
      (SymbolType::Field, "focus"),
      (SymbolType::Field, "findScope"),
      (SymbolType::Field, "matchIndex"),
    ]
  );

  let match_kind = items
    .iter()
    .find(|item| item.entry.name == "MatchKind")
    .expect("enum should be extracted");
  let enum_members = match_kind
    .members
    .iter()
    .map(|member| member.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(enum_members, vec!["Exact", "Fuzzy"]);

  let contrib = items
    .iter()
    .find(|item| item.entry.name == "NotebookFindContrib")
    .expect("class should be extracted");
  let class_members = contrib
    .members
    .iter()
    .map(|member| {
      (
        member.entry.symbol_type,
        member.entry.name.as_ref(),
        member.is_public,
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    class_members,
    vec![
      (SymbolType::Field, "id", true),
      (SymbolType::Field, "_widget", false),
      (SymbolType::Constructor, "constructor", true),
      (SymbolType::Method, "widget", true),
      (SymbolType::Method, "hide", true),
      (SymbolType::Method, "_reset", false),
    ]
  );

  let widget = items
    .iter()
    .find(|item| item.entry.name == "NotebookFindWidget")
    .expect("non-exported class should be extracted");
  assert!(!widget.is_exported);
  assert_eq!(
    widget
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>(),
    vec![("_findWidgetVisible", false), ("show", true)]
  );
}

#[test]
fn extracts_tsx_outline_with_jsx_values() {
  let combined = compile_for(SupportLang::Tsx);
  let grep = SupportLang::Tsx.ast_grep(
    r#"
import React from 'react';

export interface BadgeProps {
  title: string;
}

export function Badge(props: BadgeProps) {
  return <span>{props.title}</span>;
}

export class Panel extends React.Component<BadgeProps> {
  render() {
    return <Badge title="demo" />;
  }
}

export const BadgeFactory = () => <Badge title="demo" />;

const renderBadge = (title: string) => <Badge title={title} />;
const renderElementShape = (element: ExcalidrawElement) => {
  return <Badge title={element.id} />;
};
let currentRenderer: React.FC<BadgeProps>;
"#,
  );

  let items = combined.extract(grep.root());
  let names = items
    .iter()
    .map(|item| item.entry.name.as_ref())
    .collect::<Vec<_>>();

  assert!(
    items
      .iter()
      .any(|item| item.is_import && item.entry.name == "'react'")
  );
  assert!(names.contains(&"BadgeProps"));
  assert!(names.contains(&"Badge"));
  assert!(names.contains(&"Panel"));
  assert!(names.contains(&"BadgeFactory"));
  assert!(names.contains(&"renderBadge"));
  assert!(names.contains(&"renderElementShape"));
  assert!(names.contains(&"currentRenderer"));

  let top_level_symbols = items
    .iter()
    .map(|item| {
      (
        item.entry.name.as_ref(),
        item.entry.symbol_type,
        item.is_exported,
      )
    })
    .collect::<Vec<_>>();
  assert!(top_level_symbols.contains(&("renderBadge", SymbolType::Function, false)));
  assert!(top_level_symbols.contains(&("renderElementShape", SymbolType::Function, false)));
  assert!(top_level_symbols.contains(&("currentRenderer", SymbolType::Variable, false)));

  let panel = items
    .iter()
    .find(|item| item.entry.name == "Panel")
    .expect("TSX class should be extracted");
  assert_eq!(
    panel
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>(),
    vec!["render"]
  );
}
