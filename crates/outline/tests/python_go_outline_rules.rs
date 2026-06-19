use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_language::SupportLang;
use ast_grep_outline::{
  combined_extractor::CombinedExtractors, extractor::parse_outline_rules, model::SymbolType,
};

fn combined(src: &str) -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(src).expect("outline rules should deserialize");
  CombinedExtractors::try_from(rules, &Default::default()).expect("outline rules should compile")
}

#[test]
fn python_rules_extract_imports_classes_functions_and_methods() {
  let combined = combined(include_str!("../src/default_rules/python.yml"));
  let grep = SupportLang::Python.ast_grep(
    r#"
import functools
from django.db import models

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"
urlpatterns = []

def module_helper(queryset):
    local_count = queryset.count()
    return queryset.count()

async def fetch_related(queryset):
    return [row async for row in queryset]

class QuerySetRunner:
    empty_result_set_value = None
    query = models.Query()

    @property
    def db(self):
        return self.query.db

    class Meta:
        app_label = "outline"

    def __init__(self, queryset):
        self.queryset = queryset

    async def execute(self):
        local_value = 1
        return await self.queryset.afirst()
"#,
  );

  let items = combined.extract(grep.root());
  let names = items
    .iter()
    .map(|item| (item.entry.symbol_type, item.entry.name.as_ref()))
    .collect::<Vec<_>>();

  assert_eq!(
    names,
    vec![
      (SymbolType::Module, "functools"),
      (SymbolType::Module, "django.db.models"),
      (SymbolType::Constant, "DEFAULT_AUTO_FIELD"),
      (SymbolType::Variable, "urlpatterns"),
      (SymbolType::Function, "module_helper"),
      (SymbolType::Function, "fetch_related"),
      (SymbolType::Class, "QuerySetRunner")
    ]
  );

  let class = items
    .iter()
    .find(|item| item.entry.name == "QuerySetRunner")
    .expect("class should be extracted");
  let methods = class
    .members
    .iter()
    .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
    .collect::<Vec<_>>();
  assert_eq!(
    methods,
    vec![
      (SymbolType::Field, "empty_result_set_value"),
      (SymbolType::Field, "query"),
      (SymbolType::Method, "db"),
      (SymbolType::Class, "Meta"),
      (SymbolType::Method, "__init__"),
      (SymbolType::Method, "execute")
    ]
  );
}

#[test]
fn go_rules_extract_imports_funcs_methods_types_consts_and_vars() {
  let combined = combined(include_str!("../src/default_rules/go.yml"));
  let grep = SupportLang::Go.ast_grep(
    r#"
package gin

import (
    "net/http"
    "strings"
)

const defaultMultipartMemory = 32 << 20
var defaultTrustedCIDRs = []*net.IPNet{}

type HandlerFunc func(*Context)

type Engine struct {
    RouterGroup
    trees methodTrees
    maxParams uint16
    Config struct {
        Inner int
    }
}

type RoutesInfo interface {
    Last() RouteInfo
    ByName(string) (RouteInfo, bool)
    http.Handler
}

func New() *Engine {
    return &Engine{}
}

func (engine *Engine) Use(middleware ...HandlerFunc) IRoutes {
    return engine
}
"#,
  );

  let items = combined.extract(grep.root());
  let names = items
    .iter()
    .map(|item| (item.entry.symbol_type, item.entry.name.as_ref()))
    .collect::<Vec<_>>();

  assert_eq!(
    names,
    vec![
      (SymbolType::Module, "net/http"),
      (SymbolType::Module, "strings"),
      (SymbolType::Constant, "defaultMultipartMemory"),
      (SymbolType::Variable, "defaultTrustedCIDRs"),
      (SymbolType::TypeParameter, "HandlerFunc"),
      (SymbolType::Struct, "Engine"),
      (SymbolType::Interface, "RoutesInfo"),
      (SymbolType::Function, "New"),
      (SymbolType::Method, "Use")
    ]
  );

  let engine = items
    .iter()
    .find(|item| item.entry.name == "Engine")
    .expect("struct should be extracted");
  let fields = engine
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
    fields,
    vec![
      (SymbolType::Field, "trees", false),
      (SymbolType::Field, "maxParams", false),
      (SymbolType::Field, "Config", true)
    ]
  );

  let routes = items
    .iter()
    .find(|item| item.entry.name == "RoutesInfo")
    .expect("interface should be extracted");
  let interface_methods = routes
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
    interface_methods,
    vec![
      (SymbolType::Method, "Last", true),
      (SymbolType::Method, "ByName", true)
    ]
  );
}
