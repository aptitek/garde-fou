//! Starter project templates for `garde init --lang go`, `--lang kotlin`, and `--lang csharp`.
//!
//! Every generated starter includes a real unit test and passes `garde check --no-cache` with 0 violations.

use gf_core::{LanguageId, ProjectContext};

const GO_APP: &str = "package app\n\n// Sum returns the sum of two integers.\nfunc Sum(a, b int) int {\n\treturn a + b\n}\n";
const GO_TEST: &str = "package app\n\nimport \"testing\"\n\nfunc TestSum(t *testing.T) {\n\tif got := Sum(2, 3); got != 5 {\n\t\tt.Fatalf(\"expected 5, got %d\", got)\n\t}\n}\n";
const KT_GRADLE: &str = "plugins {\n    kotlin(\"jvm\") version \"2.1.0\"\n}\n\nrepositories {\n    mavenCentral()\n}\n";
const CS_PROJ: &str = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net9.0</TargetFramework>\n    <Nullable>enable</Nullable>\n    <TreatWarningsAsErrors>true</TreatWarningsAsErrors>\n    <EnableNETAnalyzers>true</EnableNETAnalyzers>\n  </PropertyGroup>\n</Project>\n";
const CS_APP: &str = "namespace App.Core;\n\npublic static class MathOps\n{\n    public static int Sum(int a, int b) => a + b;\n}\n";
const CS_TEST: &str = "using App.Core;\nusing Xunit;\n\nnamespace App.Tests;\n\npublic sealed class AppTests\n{\n    [Fact]\n    public void SumAddsNumbers()\n    {\n        Assert.Equal(5, MathOps.Sum(2, 3));\n    }\n}\n";

/// Appends idiomatic Go, Kotlin, and C# starter files when those languages are active in `ctx`.
pub fn push_compiled_lang_templates(
    project_name: &str,
    pkg_snake: &str,
    ctx: &ProjectContext,
    files: &mut Vec<(String, String)>,
) {
    if ctx.languages.contains(&LanguageId::Go) {
        files.extend([
            (
                String::from("go.mod"),
                format!("module example.com/{project_name}\n\ngo 1.24\n"),
            ),
            (String::from("internal/app/app.go"), String::from(GO_APP)),
            (
                String::from("internal/app/app_test.go"),
                String::from(GO_TEST),
            ),
        ]);
    }
    if ctx.languages.contains(&LanguageId::Kotlin) {
        files.extend([
            (String::from("build.gradle.kts"), String::from(KT_GRADLE)),
            (
                String::from("src/main/kotlin/App.kt"),
                format!("package com.example.{pkg_snake}\n\nfun sum(a: Int, b: Int): Int = a + b\n"),
            ),
            (
                String::from("src/test/kotlin/AppTest.kt"),
                format!(
                    "package com.example.{pkg_snake}\n\nimport kotlin.test.Test\nimport kotlin.test.assertEquals\n\nclass AppTest {{\n    @Test\n    fun sumAddsTwoNumbers() {{\n        assertEquals(5, sum(2, 3))\n    }}\n}}\n"
                ),
            ),
        ]);
    }
    if ctx.languages.contains(&LanguageId::CSharp) {
        files.extend([
            (format!("{pkg_snake}.csproj"), String::from(CS_PROJ)),
            (String::from("src/App.cs"), String::from(CS_APP)),
            (String::from("tests/AppTests.cs"), String::from(CS_TEST)),
        ]);
    }
}
