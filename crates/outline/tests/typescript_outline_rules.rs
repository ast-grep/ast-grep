use ast_grep_language::SupportLang;

mod common;

const TYPESCRIPT_RULES: &str = include_str!("../src/default_rules/typescript.yml");

#[test]
fn immediate_item_rules_only_match_direct_children() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
stopBy: immediate
rule:
  kind: function_declaration
  has:
    field: name
    pattern: $NAME
name: $NAME
isExported: false
"#,
    r#"
function direct() {}
if (ready) {
  function nested() {}
}
"#,
    r#"
- Function item private direct
"#,
  );
}

#[test]
fn mixed_item_rules_keep_end_traversal_and_direct_child_precedence() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    r#"
id: ts-immediate-function
language: TypeScript
role: item
symbolType: function
stopBy: immediate
rule:
  kind: function_declaration
  has:
    field: name
    pattern: $NAME
name: immediate-$NAME
isExported: false
---
id: ts-end-function
language: TypeScript
role: item
symbolType: function
stopBy: end
rule:
  kind: function_declaration
  has:
    field: name
    pattern: $NAME
name: end-$NAME
isExported: false
"#,
    r#"
function direct() {}
if (ready) {
  function nested() {}
}
"#,
    r#"
- Function item private immediate-direct
- Function item private end-nested
"#,
  );
}

#[test]
fn immediate_member_rules_only_match_direct_children() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    r#"
id: ts-class-body
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_body
name: body
isExported: false
---
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class-body]
symbolType: method
stopBy: immediate
rule:
  kind: method_definition
  has:
    field: name
    pattern: $NAME
name: $NAME
"#,
    r#"
class Outer {
  direct() {}
  field = class Inner {
    nested() {}
  };
}
"#,
    r#"
- Class item private body
  - Method public direct
"#,
  );
}

#[test]
fn mixed_member_rules_keep_end_traversal_and_direct_child_precedence() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    r#"
id: ts-class-body
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_body
name: body
isExported: false
---
id: ts-immediate-method
language: TypeScript
role: member
parentRuleIds: [ts-class-body]
symbolType: method
stopBy: immediate
rule:
  kind: method_definition
  has:
    field: name
    pattern: $NAME
name: immediate-$NAME
---
id: ts-end-method
language: TypeScript
role: member
parentRuleIds: [ts-class-body]
symbolType: method
stopBy: end
rule:
  kind: method_definition
  has:
    field: name
    pattern: $NAME
name: end-$NAME
"#,
    r#"
class Outer {
  direct() {}
  field = class Inner {
    nested() {}
  };
}
"#,
    r#"
- Class item private body
  - Method public immediate-direct
  - Method public end-nested
"#,
  );
}

#[test]
fn extracts_typescript_outline_from_realistic_vscode_style_code() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    TYPESCRIPT_RULES,
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
let typedArrow: () => void = () => {};
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
    r#"
- Module import private '../../../../../../base/browser/dom.js'
- Module import private '../../../../../../base/common/lazy.js'
- Module import private '../../notebookBrowser.js'
- Module item exported exports
- Module item exported '../../../common/notebookCommon.js'
- Constant item private FIND_SHOW_TRANSITION
- Constant item exported FIND_HIDE_TRANSITION
- Variable item exported MAX_MATCHES_COUNT_WIDTH
- Variable item private validateIndicesThrottled
- Function item private typedArrow
- Function item private getNonDeletedElements
- Function item private hashSelectionOpts
- Function item private noop
- Constant item private _RPCProtocolSymbol
- Interface item exported IShowNotebookFindWidgetOptions
  - Field public focus
  - Field public findScope
  - Field public matchIndex
- Struct item exported FindDirection
- Enum item exported MatchKind
  - EnumMember public Exact
  - EnumMember public Fuzzy
- Function item exported createFindWidget
- Function item private localHelper
- Class item exported NotebookFindContrib
  - Field public id
  - Field private _widget
  - Constructor public constructor
  - Method public widget
  - Method public hide
  - Method private _reset
- Class item private NotebookFindWidget
  - Field private _findWidgetVisible
  - Method public show
"#,
  );
}

#[test]
fn extracts_tsx_outline_with_jsx_values() {
  common::assert_outline_snapshot(
    SupportLang::Tsx,
    TYPESCRIPT_RULES,
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
let typedRenderer: React.FC<BadgeProps> = () => <Badge title="typed" />;
"#,
    r#"
- Module import private 'react'
- Interface item exported BadgeProps
  - Field public title
- Function item exported Badge
- Class item exported Panel
  - Method public render
- Constant item exported BadgeFactory
- Function item private renderBadge
- Function item private renderElementShape
- Variable item private currentRenderer
- Function item private typedRenderer
"#,
  );
}

#[test]
fn extracts_typescript_duplicate_member_names_with_signatures() {
  common::assert_outline_signature_snapshot(
    SupportLang::TypeScript,
    TYPESCRIPT_RULES,
    r#"
export interface Parser {
  parse(input: string): Node;
  parse(input: Uint8Array): Node;
  reset(): void;
}
"#,
    r#"
- Interface item exported Parser | export interface Parser {
  - Method public parse | parse(input: string): Node
  - Method public parse | parse(input: Uint8Array): Node
  - Method public reset | reset(): void
"#,
  );
}

#[test]
fn extracts_typescript_namespaces_as_standalone_items() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    TYPESCRIPT_RULES,
    r#"
export namespace PublicApi {
  export interface Options {}
  export function create() {}
}

namespace Local.Tools {
  export class Helper {}
}

declare namespace AmbientApi {
  export interface Client {}
}

export declare namespace ExportedAmbientApi {
  export type Result = string;
}
"#,
    r#"
- Module item exported PublicApi
- Module item private Local.Tools
- Module item exported AmbientApi
- Module item exported ExportedAmbientApi
"#,
  );
}

#[test]
fn extracts_typescript_ambient_modules_as_standalone_items() {
  common::assert_outline_snapshot(
    SupportLang::TypeScript,
    TYPESCRIPT_RULES,
    r#"
declare module "plain-package" {
  export interface Resource {}
}

export declare module "exported-package" {
  export type API = string;
}
"#,
    r#"
- Module item exported plain-package
- Module item exported exported-package
"#,
  );
}
