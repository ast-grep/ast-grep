use ast_grep_language::SupportLang;

mod common;

const TYPESCRIPT_RULES: &str = include_str!("../src/default_rules/typescript.yml");

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
