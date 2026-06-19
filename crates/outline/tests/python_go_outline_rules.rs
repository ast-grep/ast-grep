use ast_grep_language::SupportLang;

mod common;

#[test]
fn python_rules_extract_imports_classes_functions_and_methods() {
  common::assert_outline_snapshot(
    SupportLang::Python,
    include_str!("../src/default_rules/python.yml"),
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

class AsyncOnly:
    async def run(self):
        return None
"#,
    r#"
- Module import private functools
- Module import private django.db.models
- Constant item exported DEFAULT_AUTO_FIELD
- Variable item exported urlpatterns
- Function item exported module_helper
- Function item exported fetch_related
- Class item exported QuerySetRunner
  - Field public empty_result_set_value
  - Field public query
  - Method public db
  - Class public Meta
  - Method public __init__
  - Method public execute
- Class item exported AsyncOnly
  - Method public run
"#,
  );
}

#[test]
fn go_rules_extract_imports_funcs_methods_types_consts_and_vars() {
  common::assert_outline_snapshot(
    SupportLang::Go,
    include_str!("../src/default_rules/go.yml"),
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
    r#"
- Module import private net/http
- Module import private strings
- Constant item exported defaultMultipartMemory
- Variable item exported defaultTrustedCIDRs
- TypeParameter item exported HandlerFunc
- Struct item exported Engine
  - Field private trees
  - Field private maxParams
  - Field public Config
- Interface item exported RoutesInfo
  - Method public Last
  - Method public ByName
- Function item exported New
- Method item exported Use
"#,
  );
}
