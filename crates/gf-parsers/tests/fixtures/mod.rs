//! Raw Python and `pyproject.toml` source fixtures for `python_ast_corpus`.

pub const CLEAN_SECURITY_PY: &str = "\
import logging, subprocess, yaml
logger = logging.getLogger(__name__)
def run_safe(stream: str) -> object:
    data: object = yaml.load(stream, Loader=yaml.SafeLoader)
    subprocess.run([\"ls\", \"-la\"], check=True)
    try:
        return data
    except ValueError as exc:
        logger.warning(\"invalid yaml\")
        raise RuntimeError(\"bad\") from exc
";

pub const DIRTY_SECURITY_PY: &str = "\
import pdb, pickle, subprocess, typing, yaml
from typing import Any
def hazard(payload: Any, stream: str, raw: bytes) -> object:
    casted = typing.cast(Any, stream)
    eval(stream)
    pickle.loads(raw)
    yaml.load(stream)
    subprocess.run(stream, shell=True)
    print(casted)
    breakpoint()
    pdb.set_trace()
    try:
        return payload
    except ValueError:
        pass
    except:
        raise
";

pub const DATA_MODELS_IMMUNITY_PY: &str = "\
\"\"\"eval(x) # noqa # type: ignore print(1) subprocess.run(x, shell=True)\"\"\"
from dataclasses import dataclass
from enum import Enum
from typing import NamedTuple, TypedDict
from pydantic import BaseModel
@dataclass(frozen=True)
class UserDto:
    user_id: int
class OrderDict(TypedDict):
    total: int
class Coord(NamedTuple):
    x: int
class Role(Enum):
    ADMIN = \"admin\"
class InvoiceSchema(BaseModel):
    cents: int
def literal_immunity() -> str:
    return \"eval(x) # noqa # type: ignore # pragma: no cover # pyright: ignore print(1) subprocess.run(x, shell=True)\"
";

pub const WEAK_PYPROJECT: &str = "\
[tool.mypy]
strict = false
warn_return_any = false
[[tool.mypy.overrides]]
module = \"billing.*\"
ignore_errors = true
[tool.ruff.lint]
extend-ignore = [\"ALL\"]
select = []
[tool.pytest.ini_options]
addopts = \"-q --no-cov\"
[tool.coverage.report]
fail_under = 70
";
