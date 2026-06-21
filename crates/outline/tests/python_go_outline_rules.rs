use ast_grep_language::SupportLang;

mod common;

const PYTHON_RULES: &str = include_str!("../src/default_rules/python.yml");
const GO_RULES: &str = include_str!("../src/default_rules/go.yml");

#[test]
fn python_rules_extract_imports_classes_functions_and_methods() {
  common::assert_outline_snapshot(
    SupportLang::Python,
    PYTHON_RULES,
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
fn python_rules_extract_signatures() {
  common::assert_outline_signature_snapshot(
    SupportLang::Python,
    PYTHON_RULES,
    r#"
from django.db import models

async def fetch_related(queryset):
    return [row async for row in queryset]

class QuerySetRunner:
    query = models.Query()

    @property
    def db(self):
        return self.query.db

    async def execute(self):
        return await self.queryset.afirst()
"#,
    r#"
- Module import private django.db.models | from django.db import models
- Function item exported fetch_related | async def fetch_related(queryset):
- Class item exported QuerySetRunner | class QuerySetRunner:
  - Field public query | query = models.Query()
  - Method public db | def db(self):
  - Method public execute | async def execute(self):
"#,
  );
}

#[test]
fn go_rules_extract_imports_funcs_methods_types_consts_and_vars() {
  common::assert_outline_snapshot(
    SupportLang::Go,
    GO_RULES,
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

#[test]
fn go_rules_extract_signatures() {
  common::assert_outline_signature_snapshot(
    SupportLang::Go,
    GO_RULES,
    r#"
package gin

import "net/http"

type Engine struct {
    RouterGroup
    maxParams uint16
}

type RoutesInfo interface {
    Last() RouteInfo
    byName(string) (RouteInfo, bool)
}

func New() *Engine {
    return &Engine{}
}

func (engine *Engine) Use(middleware ...HandlerFunc) IRoutes {
    return engine
}
"#,
    r#"
- Module import private net/http | "net/http"
- Struct item exported Engine | Engine struct {
  - Field private maxParams | maxParams uint16
- Interface item exported RoutesInfo | RoutesInfo interface {
  - Method public Last | Last() RouteInfo
  - Method private byName | byName(string) (RouteInfo, bool)
- Function item exported New | func New() *Engine {
- Method item exported Use | func (engine *Engine) Use(middleware ...HandlerFunc) IRoutes {
"#,
  );
}
