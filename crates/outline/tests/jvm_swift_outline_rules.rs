use ast_grep_language::SupportLang;

mod common;

#[test]
fn kotlin_rules_parse_and_extract_okhttp_shapes() {
  const RULES: &str = include_str!("../src/default_rules/kotlin.yml");
  common::assert_rules_compile(RULES);

  let combined = common::compile_rules(RULES);
  common::assert_outline_snapshot(
    SupportLang::Kotlin,
    &combined,
    r#"
package okhttp3.sse

import okhttp3.Request
import java.io.IOException as IOE

public interface Interceptor {
  fun intercept(chain: Chain): Response
}

@Serializable
sealed class AnnotatedBox

fun interface FunctionalApi {
  fun invoke(): Unit
}

value class UserId(val value: String)

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
    r#"
- Module import private okhttp3.Request
- Module import private IOE
- Interface item exported Interceptor
  - Method public intercept
- Class item exported AnnotatedBox
- Interface item exported FunctionalApi
  - Method public invoke
- Class item exported UserId
- Class item exported RealInterceptorChain
  - Constructor private constructor
  - Method public proceed
  - Method public exchange
  - Property public index
  - Method private copy
  - Method public withConnectTimeout
- Object item exported EventSources
  - Method public createFactory
- Enum item exported Protocol
- Class item exported RealCall
  - Method public execute
  - Method public isExecuted
  - Method private callStart
  - Method private getResponseWithInterceptorChain
- Class item exported ScopeBox
  - Property public direct
  - Class public Nested
  - Object public companion object
  - Method public directFun
"#,
  );
}

#[test]
fn java_rules_parse_and_extract_jvm_surface() {
  const RULES: &str = include_str!("../src/default_rules/java.yml");
  common::assert_rules_compile(RULES);

  let combined = common::compile_rules(RULES);
  common::assert_outline_snapshot(
    SupportLang::Java,
    &combined,
    r#"
package okhttp3;

import okhttp3.Request;
import static java.util.Collections.emptyList;

public interface Interceptor {
  Response intercept(Chain chain) throws IOException;
}

@Deprecated public interface AnnotatedApi {
  static public Response reordered();
}

public final class RealInterceptorChain implements Interceptor {
  private final int index;
  @Deprecated public final int publicFinalField;
  final int packagePrivateFinalField;
  public RealInterceptorChain(int index) { this.index = index; }
  public Response proceed(Request request) throws IOException { return null; }
  Response exchange() { return null; }
}

enum Protocol { HTTP_1_1 }
"#,
    r#"
- Module import private okhttp3.Request
- Module import private java.util.Collections.emptyList
- Interface item exported Interceptor
  - Method private intercept
- Interface item exported AnnotatedApi
  - Method public reordered
- Class item exported RealInterceptorChain
  - Field public publicFinalField
  - Field private packagePrivateFinalField
  - Constructor public RealInterceptorChain
  - Method public proceed
  - Method private exchange
- Enum item private Protocol
"#,
  );
}

#[test]
fn swift_rules_parse_and_extract_alamofire_shapes() {
  const RULES: &str = include_str!("../src/default_rules/swift.yml");
  common::assert_rules_compile(RULES);

  let combined = common::compile_rules(RULES);
  common::assert_outline_snapshot(
    SupportLang::Swift,
    &combined,
    r#"
import Foundation
@preconcurrency import Dispatch

public final class Session {
  public init(configuration: URLSessionConfiguration) {}
  open func request(_ convertible: any URLConvertible) -> DataRequest { fatalError() }
}

@MainActor public final class ActorSession {
  @MainActor public func refresh() {}
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

@frozen public struct Tagged {
  public init() {}
}

@MainActor public enum AnnotatedError {
  case failed
}

@MainActor public actor Loader {
  public init() {}
  public func load() {}
}

@MainActor public func makeSession() {}

class InternalBox {
  init() {}
  func helper() -> Int { return 1 }
}
"#,
    r#"
- Module import private Foundation
- Module import private Dispatch
- Class item exported Session
  - Constructor public init
  - Method public request
- Class item exported ActorSession
  - Method public refresh
- Interface item exported RequestInterceptor
  - Method public adapt
- Enum item exported AFError
- Struct item exported HTTPHeaders
  - Constructor public init
- Struct item exported Tagged
  - Constructor public init
- Enum item exported AnnotatedError
- Object item exported Loader
  - Constructor public init
  - Method public load
- Function item exported makeSession
- Class item private InternalBox
  - Constructor private init
  - Method private helper
"#,
  );
}
