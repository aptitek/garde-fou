//! Symmetric `(invalid, subtle_valid)` corpus for Django 5.2/6.0, `NiceGUI`,
//! `PyTorch`/`Keras 3`/`scikit-learn` (including cross-cell `.ipynb` data leakage),
//! and `LangChain`/`LangGraph` (`GF080`).

use gf_core::{DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, RepoPath};
use gf_parsers::parse_source;

fn codes(path: &str, src: &str) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(|f| (String::from(f.code()), f.span.start_line()))
        .collect())
}

fn has(list: &[(String, u32)], code: &str, line: u32) -> bool {
    list.iter().any(|(c, l)| c == code && *l == line)
}

fn count_prefix(path: &str, src: &str, prefix: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(codes(path, src)?
        .into_iter()
        .filter(|(c, _)| c.starts_with(prefix))
        .count())
}

#[test]
fn django_corpus_enforces_dj001_exemption_dj008_format_html_and_runsql()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
from django.db import models
from django.utils.html import format_html
class Article(models.Model):
    slug = models.CharField(max_length=80, null=True)
def render_article(user, s):
    Article.objects.raw(f\"SELECT * FROM app_article WHERE slug = '{s}'\")
    return format_html(f\"<span>{user.name}</span>\")
";
    let f = codes("src/blog/models.py", bad)?;
    assert!(has(&f, DjangoViolation::ModelAndFormHygiene.code(), 3));
    assert!(has(&f, DjangoViolation::ModelAndFormHygiene.code(), 4));
    assert!(has(&f, DjangoViolation::RawSqlOrUnsafeHtml.code(), 6));
    assert!(has(&f, DjangoViolation::RawSqlOrUnsafeHtml.code(), 7));

    let good = "\
from django.db import models
from django.utils.html import format_html
class Article(models.Model):
    slug = models.CharField(max_length=80, unique=True, blank=True, null=True)
    def __str__(self) -> str:
        return self.slug or ''
def render_article(user: object, s: str) -> str:
    _ = Article.objects.filter(slug=s).select_related('author')[:25]
    return format_html('<span>{}</span>', user)
";
    assert_eq!(count_prefix("src/blog/good.py", good, "DJ-")?, 0);

    let bad_mig = "from django.db import migrations\nclass Migration(migrations.Migration):\n    operations = [migrations.RunSQL(f\"DELETE FROM {tbl}\")]\n";
    assert!(has(
        &codes("blog/migrations/0001_initial.py", bad_mig)?,
        DjangoViolation::RawSqlOrUnsafeHtml.code(),
        3
    ));
    Ok(())
}

#[test]
fn nicegui_corpus_enforces_event_loop_global_ui_and_worker_context()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
from nicegui import ui, app, run
import time, requests
ui.label(\"shared\")
def worker():
    app.storage.user[\"k\"] = 1
    ui.notify(\"ok\")
@ui.page(\"/\")
async def index():
    time.sleep(2)
    requests.get(\"https://example.com\")
    await run.io_bound(worker)
";
    let f = codes("src/app.py", bad)?;
    assert!(has(&f, NiceGuiViolation::GlobalUiOrStateLeak.code(), 3));
    assert!(has(&f, NiceGuiViolation::WorkerContextViolation.code(), 5));
    assert!(has(&f, NiceGuiViolation::WorkerContextViolation.code(), 6));
    assert!(has(&f, NiceGuiViolation::BlockingEventLoop.code(), 9));
    assert!(has(&f, NiceGuiViolation::BlockingEventLoop.code(), 10));

    let good = "\
from nicegui import ui, app, run
def compute(val: int) -> int:
    return val * 2
@ui.page(\"/\")
async def index() -> None:
    val = int(app.storage.user.get(\"n\", 1))
    res = await run.io_bound(compute, val)
    ui.label(str(res))
";
    assert_eq!(count_prefix("src/good_ui.py", good, "NICEGUI-")?, 0);
    Ok(())
}

#[test]
fn pytorch_keras_and_cross_cell_notebook_data_leakage_corpus()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
import torch, keras
torch.load(\"m.pt\", weights_only=False)
keras.saving.load_model(\"m.keras\", safe_mode=False)
keras.saving.load_model(\"legacy.h5\")
for x in loader:
    loss = model(x)
    loss.backward()
    optimizer.step()
    history.append(loss)
";
    let f = codes("src/train.py", bad)?;
    let deser = MlDlViolation::UnsafeModelDeserialization.code();
    let vram = MlDlViolation::TorchTrainingAndVramBugs.code();
    assert!(has(&f, deser, 2) && has(&f, deser, 3) && has(&f, deser, 4));
    assert!(has(&f, vram, 5) && has(&f, vram, 9));

    let good = "\
import torch, keras
torch.load(\"m.pt\", map_location=\"cpu\", weights_only=True)
keras.saving.load_model(\"m.keras\")
for x in loader:
    optimizer.zero_grad(set_to_none=True)
    loss = model(x)
    loss.backward()
    optimizer.step()
    history.append(loss.item())
";
    assert_eq!(count_prefix("src/good_train.py", good, "ML-")?, 0);

    let nb_leak = "{\n  \"cells\": [\n    {\"cell_type\": \"code\", \"execution_count\": 1, \"metadata\": {}, \"outputs\": [], \"source\": [\"X_scaled = scaler.fit_transform(X)\\n\"]},\n    {\"cell_type\": \"code\", \"execution_count\": 2, \"metadata\": {}, \"outputs\": [], \"source\": [\"X_tr, X_te = train_test_split(X_scaled)\\n\"]}\n  ],\n  \"metadata\": {}, \"nbformat\": 4, \"nbformat_minor\": 5\n}\n";
    let nb_f = codes("notebooks/leak.ipynb", nb_leak)?;
    assert!(
        nb_f.iter()
            .any(|(c, _)| c == MlDlViolation::DataLeakageBeforeSplit.code())
    );
    Ok(())
}

#[test]
fn langchain_and_langgraph_rce_and_unbounded_graph_corpus() -> Result<(), Box<dyn std::error::Error>>
{
    let bad = "\
from langchain_core.prompts import PromptTemplate
from langgraph.graph import StateGraph
PromptTemplate.from_template(t, template_format=\"jinja2\")
load_prompt(\"p.yaml\")
PythonREPLTool()
loads(raw, secrets_from_env=True)
g = StateGraph(State)
g.add_conditional_edges(\"a\", router)
app = g.compile()
app.ainvoke(state)
";
    let f = codes("src/agent.py", bad)?;
    let rce = LlmAgentViolation::RceAndCveVectors.code();
    let graph = LlmAgentViolation::LangGraphInfiniteLoopAndBudget.code();
    assert!(has(&f, rce, 3) && has(&f, rce, 4) && has(&f, rce, 5) && has(&f, rce, 6));
    assert!(has(&f, graph, 9) && has(&f, graph, 10));

    let good = "\
from langchain_core.prompts import ChatPromptTemplate
from langgraph.graph import StateGraph, END
p = ChatPromptTemplate.from_messages([(\"system\", \"Be concise.\"), (\"human\", \"{q}\")])
g = StateGraph(State)
g.add_conditional_edges(\"a\", router)
g.add_edge(\"a\", END)
app = g.compile()
app.ainvoke(state, config={\"recursion_limit\": 12})
";
    assert_eq!(count_prefix("src/good_agent.py", good, "LLM-")?, 0);
    Ok(())
}
