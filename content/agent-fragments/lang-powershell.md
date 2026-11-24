# PowerShell Automation Standards (`GF080`–`GF082`)

## Strict Header, Null Safety & Secret Parameters

- Declare `Set-StrictMode -Version Latest` and `$ErrorActionPreference = 'Stop'` at the top of executable `.ps1` scripts (`PS-STRICT-HEADER`, `GF080`).
- Place `$null` on the left-hand side of equality comparisons (`$null -eq $items`, `$null -ne $cmd`) so array operands never trigger PowerShell collection filtering (`PS-NULL-LEFT`).
- Forbid `Invoke-Expression` / `iex` (`PS-NO-IEX`), type sensitive `param(...)` credentials (`$Password`, `$Secret`, `$ApiKey`, `$AccessToken`, `$PrivateKey`) as `[SecureString]` or `[PSCredential]` instead of `[string]` (`PS-PLAIN-SECRET-PARAM`), handle or log errors inside `catch` blocks (`PS-EMPTY-CATCH`), and avoid `$global:...` state (`PS-GLOBAL-VAR`).
- Never silence `PSScriptAnalyzer` via unjustified `[Diagnostics.CodeAnalysis.SuppressMessageAttribute(...)]` (requires `Justification` `>= 10` characters, `GF070`) or `ExcludeRules = @('*')` in `PSScriptAnalyzerSettings.psd1` (`GF081`), and resolve `[psscriptanalyzer:...]` diagnostics (`GF082`).
