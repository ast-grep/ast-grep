use ast_grep_language::{LanguageExt, SupportLang};
use ast_grep_outline::{
  combined_extractor::CombinedExtractors,
  extractor::{SerializableOutlineRule, parse_outline_rules},
  model::SymbolType,
};

fn compile(rules: &'static str) -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(rules).expect("outline YAML should parse");
  CombinedExtractors::try_from(rules, &Default::default()).expect("outline YAML should compile")
}

fn parse_all(rules: &'static str) {
  let rules = parse_outline_rules::<SupportLang>(rules).expect("outline YAML should parse");
  for rule in rules {
    match rule {
      SerializableOutlineRule::Item(item) => {
        ast_grep_outline::extractor::ItemExtractor::try_from(item, &Default::default())
          .expect("item rule should compile");
      }
      SerializableOutlineRule::Member(member) => {
        ast_grep_outline::extractor::MemberExtractor::try_from(member, &Default::default())
          .expect("member rule should compile");
      }
    }
  }
}

#[test]
fn kotlin_rules_parse_and_extract_okhttp_shapes() {
  const RULES: &str = include_str!("../src/default_rules/kotlin.yml");
  parse_all(RULES);

  let combined = compile(RULES);
  let grep = SupportLang::Kotlin.ast_grep(
    r#"
package okhttp3.sse

import okhttp3.Request
import java.io.IOException as IOE

public interface Interceptor {
  fun intercept(chain: Chain): Response
}

class RealInterceptorChain constructor(val call: Call) {
  internal constructor(call: Call, index: Int) : this(call)
  override fun proceed(request: Request): Response { return TODO() }
  fun exchange(): Exchange = TODO()
  val index: Int = 0
  internal fun copy(index: Int = 0) = RealInterceptorChain(call)
  override fun withConnectTimeout(timeout: Int, unit: TimeUnit): Interceptor.Chain {
    return copy()
  }
}

public object EventSources {
  fun createFactory(): Factory = TODO()
}

enum class Protocol { HTTP_1_1 }

class RealCall {
  override fun execute(): Response {
    return getResponseWithInterceptorChain()
  }
  override fun isExecuted(): Boolean = TODO()
  private fun callStart() {}
  @Throws(IOException::class)
  internal fun getResponseWithInterceptorChain(): Response {
    val localResponse: Response = TODO()
    var localCalls: Int = 0
    return localResponse
  }
}

class ScopeBox {
  val direct: Int = 0
  class Nested {
    fun leaked(): Unit {}
    val nestedValue: Int = 0
  }
  companion object {
    fun companionLeak(): Unit {}
    val companionValue: Int = 0
  }
  fun directFun() {}
}
"#,
  );
  let items = combined.extract(grep.root());

  let names = items
    .iter()
    .map(|item| item.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(
    names,
    vec![
      "okhttp3.Request",
      "IOE",
      "Interceptor",
      "RealInterceptorChain",
      "EventSources",
      "Protocol",
      "RealCall",
      "ScopeBox"
    ]
  );

  let chain = items
    .iter()
    .find(|item| item.entry.name == "RealInterceptorChain")
    .expect("class should be outlined");
  let member_names = chain
    .members
    .iter()
    .map(|member| member.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(
    member_names,
    vec![
      "constructor",
      "proceed",
      "exchange",
      "index",
      "copy",
      "withConnectTimeout"
    ]
  );
  let chain_members = chain
    .members
    .iter()
    .map(|member| {
      (
        member.entry.name.as_ref(),
        member.entry.signature.as_ref(),
        member.entry.symbol_type,
        member.is_public,
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    chain_members,
    vec![
      (
        "constructor",
        "internal constructor(call: Call, index: Int) : this(call)",
        SymbolType::Constructor,
        false,
      ),
      (
        "proceed",
        "override fun proceed(request: Request): Response { return TODO() }",
        SymbolType::Method,
        true,
      ),
      (
        "exchange",
        "fun exchange(): Exchange = TODO()",
        SymbolType::Method,
        true
      ),
      ("index", "val index: Int = 0", SymbolType::Property, true),
      (
        "copy",
        "internal fun copy(index: Int = 0) = RealInterceptorChain(call)",
        SymbolType::Method,
        false
      ),
      (
        "withConnectTimeout",
        "override fun withConnectTimeout(timeout: Int, unit: TimeUnit): Interceptor.Chain {",
        SymbolType::Method,
        true,
      ),
    ]
  );

  let real_call = items
    .iter()
    .find(|item| item.entry.name == "RealCall")
    .expect("RealCall should be outlined");
  let real_call_members = real_call
    .members
    .iter()
    .map(|member| {
      (
        member.entry.name.as_ref(),
        member.entry.signature.as_ref(),
        member.entry.symbol_type,
        member.is_public,
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    real_call_members,
    vec![
      (
        "execute",
        "override fun execute(): Response {",
        SymbolType::Method,
        true
      ),
      (
        "isExecuted",
        "override fun isExecuted(): Boolean = TODO()",
        SymbolType::Method,
        true
      ),
      (
        "callStart",
        "private fun callStart() {}",
        SymbolType::Method,
        false
      ),
      (
        "getResponseWithInterceptorChain",
        "@Throws(IOException::class)",
        SymbolType::Method,
        false,
      ),
    ]
  );

  let scope_box = items
    .iter()
    .find(|item| item.entry.name == "ScopeBox")
    .expect("ScopeBox should be outlined");
  let scope_members = scope_box
    .members
    .iter()
    .map(|member| (member.entry.name.as_ref(), member.entry.symbol_type))
    .collect::<Vec<_>>();
  assert_eq!(
    scope_members,
    vec![
      ("direct", SymbolType::Property),
      ("Nested", SymbolType::Class),
      ("companion object", SymbolType::Object),
      ("directFun", SymbolType::Method),
    ]
  );
}

#[test]
fn java_rules_parse_and_extract_jvm_surface() {
  const RULES: &str = include_str!("../src/default_rules/java.yml");
  parse_all(RULES);

  let combined = compile(RULES);
  let grep = SupportLang::Java.ast_grep(
    r#"
package okhttp3;

import okhttp3.Request;
import static java.util.Collections.emptyList;

public interface Interceptor {
  Response intercept(Chain chain) throws IOException;
}

public final class RealInterceptorChain implements Interceptor {
  private final int index;
  public RealInterceptorChain(int index) { this.index = index; }
  public Response proceed(Request request) throws IOException { return null; }
  Response exchange() { return null; }
}

enum Protocol { HTTP_1_1 }
"#,
  );
  let items = combined.extract(grep.root());

  let names = items
    .iter()
    .map(|item| item.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(
    names,
    vec![
      "okhttp3.Request",
      "java.util.Collections.emptyList",
      "Interceptor",
      "RealInterceptorChain",
      "Protocol"
    ]
  );

  let chain = items
    .iter()
    .find(|item| item.entry.name == "RealInterceptorChain")
    .expect("class should be outlined");
  assert!(chain.is_exported);
  let members = chain
    .members
    .iter()
    .map(|member| {
      (
        member.entry.name.as_ref(),
        member.entry.symbol_type,
        member.is_public,
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    members,
    vec![
      (
        "RealInterceptorChain",
        ast_grep_outline::model::SymbolType::Constructor,
        true,
      ),
      ("proceed", ast_grep_outline::model::SymbolType::Method, true),
      (
        "exchange",
        ast_grep_outline::model::SymbolType::Method,
        false
      ),
    ]
  );
}

#[test]
fn swift_rules_parse_and_extract_alamofire_shapes() {
  const RULES: &str = include_str!("../src/default_rules/swift.yml");
  parse_all(RULES);

  let combined = compile(RULES);
  let grep = SupportLang::Swift.ast_grep(
    r#"
import Foundation
@preconcurrency import Dispatch

public final class Session {
  public init(configuration: URLSessionConfiguration) {}
  open func request(_ convertible: any URLConvertible) -> DataRequest { fatalError() }
}

public protocol RequestInterceptor: Sendable {
  func adapt(_ urlRequest: URLRequest) throws -> URLRequest
}

public enum AFError: Error {
  case invalidURL(url: any URLConvertible)
}

public struct HTTPHeaders: Sendable {
  public init(_ headers: [HTTPHeader]) {}
}

class InternalBox {
  init() {}
  func helper() -> Int { return 1 }
}
"#,
  );
  let items = combined.extract(grep.root());

  let names = items
    .iter()
    .map(|item| item.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(
    names,
    vec![
      "Foundation",
      "Dispatch",
      "Session",
      "RequestInterceptor",
      "AFError",
      "HTTPHeaders",
      "InternalBox"
    ]
  );

  let session = items
    .iter()
    .find(|item| item.entry.name == "Session")
    .expect("class should be outlined");
  assert!(session.is_exported);
  let member_names = session
    .members
    .iter()
    .map(|member| member.entry.name.as_ref())
    .collect::<Vec<_>>();
  assert_eq!(member_names, vec!["init", "request"]);
  assert!(session.members.iter().all(|member| member.is_public));

  let internal = items
    .iter()
    .find(|item| item.entry.name == "InternalBox")
    .expect("internal class should be outlined");
  assert!(!internal.is_exported);
  assert_eq!(
    internal
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>(),
    vec![("init", false), ("helper", false)]
  );
}
