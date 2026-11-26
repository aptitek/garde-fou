//! Symmetric `(invalid, subtle_valid)` corpus for Go, Kotlin, C# CST rules (`GF080`),
//! Tier 2 configs (`GF081`), and Tier 3 tool output parsers (`GF082`).

use gf_core::{
    CSharpViolation, CompiledLangViolation, GoViolation, KotlinViolation, LangRuleViolation,
    RepoPath, SqlViolation,
};
use gf_parsers::parse_source;

fn codes_and_lines(
    path: &str,
    src: &str,
) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(|f| (String::from(f.code()), f.span.start_line()))
        .collect())
}

fn has(findings: &[(String, u32)], code: &str, line: u32) -> bool {
    findings.iter().any(|(c, l)| c == code && *l == line)
}

#[test]
fn go_corpus_detects_sa4023_sa5001_defer_ctx_sql_and_excludes_receiver()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
package svc
import \"context\"
import \"fmt\"
type MyErr struct{}
func load() error {
    var e *MyErr
    return e
}
func fetchAll(id int, ctx context.Context, files []string) error {
    _ = context.Background()
    resp, err := open(files[0])
    defer resp.Body.Close()
    if err != nil { return err }
    for _, f := range files {
        fd, err := open(f)
        if err != nil { return err }
        defer fd.Close()
    }
    db.Query(fmt.Sprintf(\"SELECT * FROM users WHERE id = %d\", id))
    return nil
}
";
    let findings = codes_and_lines("internal/svc/svc.go", bad)?;
    assert!(has(&findings, GoViolation::TypedNilInterface.code(), 7));
    assert!(has(&findings, GoViolation::ContextMisuse.code(), 9));
    assert!(has(&findings, GoViolation::ContextMisuse.code(), 10));
    assert!(has(&findings, GoViolation::DeferBeforeErrCheck.code(), 12));
    assert!(has(&findings, GoViolation::DeferInLoop.code(), 17));
    assert!(has(
        &findings,
        SqlViolation::InjectionInterpolation.code(),
        19
    ));
    Ok(())
}

#[test]
fn go_corpus_excludes_method_receiver_from_param_count() -> Result<(), Box<dyn std::error::Error>> {
    let good = "\
package svc
import \"context\"
type Svc struct{}
func (s *Svc) Save(ctx context.Context, a, b, c string) error {
    resp, err := open(a)
    if err != nil { return err }
    defer resp.Body.Close()
    _, err = db.QueryContext(ctx, \"SELECT id FROM users WHERE id = $1\", b)
    if err != nil { return err }
    return nil
}
";
    let doc = parse_source(&RepoPath::new("internal/svc/good.go")?, good)?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    assert_eq!(doc.callables[0].param_count, 4);
    Ok(())
}

#[test]
fn kotlin_corpus_detects_cancellation_bang_bang_and_coroutines()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
package com.acme.app
suspend fun sync() = runCatching { fetchRemote() }
suspend fun poll() {
    try { fetchRemote() } catch (e: Exception) { log(e) }
    Thread.sleep(100)
}
suspend fun items(): Flow<String> = flow { emit(\"a\") }
fun risky(user: User?): String {
    GlobalScope.launch { poll() }
    return user!!.name
}
";
    let findings = codes_and_lines("src/main/kotlin/Bad.kt", bad)?;
    let cancel = KotlinViolation::SuspendRunCatchingCancellation.code();
    let coro = KotlinViolation::CoroutineStructuredSafety.code();
    assert!(has(&findings, cancel, 2) && has(&findings, cancel, 4));
    assert!(has(&findings, coro, 5) && has(&findings, coro, 7) && has(&findings, coro, 9));
    assert!(has(
        &findings,
        KotlinViolation::NoNotNullAssertion.code(),
        10
    ));
    Ok(())
}

#[test]
fn kotlin_corpus_exempts_sealed_adt_and_safe_coroutines() -> Result<(), Box<dyn std::error::Error>>
{
    let good = "\
package com.acme.app
sealed interface UiState { data object Loading : UiState }
suspend fun pollSafe() {
    try { fetchRemote() } catch (e: Exception) {
        if (e is CancellationException) throw e
        log(e)
    }
}
fun items(): Flow<String> = flow { delay(100); emit(\"ok\") }
fun safe(user: User?): String = user?.name ?: \"anon\"
";
    let doc = parse_source(&RepoPath::new("src/main/kotlin/Good.kt")?, good)?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    assert_eq!(doc.contracts.len(), 0);
    Ok(())
}

#[test]
fn csharp_corpus_detects_cs9124_async_void_ma0129_ca2200_and_ef_sql()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
namespace Acme.Billing;
public sealed class Billing(IRepo repo)
{
    public IRepo Repo { get; } = repo;
    public void Run() => repo.Save();
    public async void OnTick() { await StepAsync(); }
    public async Task BadAsync(CancellationToken ct)
    {
        var r = StepAsync().Result;
        using (OpenStreamAsync(ct)) { }
        try { await StepAsync(); } catch (Exception ex) { throw ex; }
        var x = ctx.Users.FromSqlRaw($\"SELECT * FROM Users WHERE Id = {r}\");
    }
}
";
    let findings = codes_and_lines("src/Billing.cs", bad)?;
    assert!(has(
        &findings,
        CSharpViolation::PrimaryCtorDoubleCapture.code(),
        5
    ));
    assert!(has(&findings, CSharpViolation::NoAsyncVoid.code(), 6));
    assert!(has(
        &findings,
        CSharpViolation::DisposeAndCancellationSafety.code(),
        7
    ));
    assert!(has(&findings, CSharpViolation::SyncOverAsync.code(), 9));
    assert!(has(
        &findings,
        CSharpViolation::DisposeAndCancellationSafety.code(),
        10
    ));
    assert!(has(
        &findings,
        CSharpViolation::RethrowDestroysStack.code(),
        11
    ));
    assert!(has(
        &findings,
        SqlViolation::InjectionInterpolation.code(),
        12
    ));
    Ok(())
}

#[test]
fn csharp_corpus_accepts_clean_async_and_excludes_extension_this_param()
-> Result<(), Box<dyn std::error::Error>> {
    let good = "\
namespace Acme.Billing;
public sealed class GoodBilling(IRepo repo)
{
    public IRepo Repo { get; } = repo;
    public void Run() => Repo.Save();
    public async Task GoodAsync(CancellationToken ct)
    {
        await using var s = await OpenStreamAsync(ct);
        try { await StepAsync(ct); } catch (Exception) { throw; }
        var x = ctx.Users.FromSql($\"SELECT * FROM Users WHERE Id = {1}\");
    }
    public static int Clamp(this int v, int a, int b, int c, int d) => v + a + b + c + d;
}
";
    let doc = parse_source(&RepoPath::new("src/GoodBilling.cs")?, good)?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    let clamp = doc
        .callables
        .iter()
        .find(|c| c.name == "Clamp")
        .ok_or("missing Clamp")?;
    assert_eq!(clamp.param_count, 4);
    assert_eq!(
        LangRuleViolation::from(CSharpViolation::NoAsyncVoid),
        LangRuleViolation::Compiled(CompiledLangViolation::CSharp(CSharpViolation::NoAsyncVoid))
    );
    Ok(())
}
