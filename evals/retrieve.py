"""Тонкая обёртка над `lumina retrieve` для eval-харнеса.

Харнес намеренно ходит через настоящий бинарь, а не переписывает скоринг
на Python: иначе мерился бы не тот код, который работает в приложении.

ВАЖНО про Windows: в release-сборке действует windows_subsystem = "windows",
консоли нет и stdout уходит в никуда. Гоняем debug — это и есть `cargo run`.
"""

import json
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CARGO = [
    "cargo", "run", "--quiet",
    "--manifest-path", str(REPO / "src-tauri" / "Cargo.toml"),
    "--",
]


def index_dir(config: str) -> str:
    """Свой индекс на каждую конфигурацию.

    Без этого прогоны затирают друг друга: chunk_250 переиндексировал бы
    базу поверх chunk_1000, и сравнивать стало бы нечего.
    """
    name = json.loads(Path(config).read_text(encoding="utf-8")).get("name", Path(config).stem)
    d = REPO / "evals" / "index" / name
    d.mkdir(parents=True, exist_ok=True)
    return str(d)


def _run(args: list[str]) -> dict:
    proc = subprocess.run(
        CARGO + args, capture_output=True, text=True, encoding="utf-8"
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "lumina завершился с ошибкой")
    lines = [l for l in proc.stdout.splitlines() if l.strip()]
    if not lines:
        raise RuntimeError("пустой stdout — проверьте, что сборка debug, а не release")
    return json.loads(lines[-1])  # cargo может печатать своё перед JSON


def retrieve(query: str, config: str, k: int = 20, data_dir: str | None = None) -> list[dict]:
    """Возвращает список хитов: chunk_id, path, char_start, char_end, score, content."""
    args = ["retrieve", "--config", config, "--query", query, "--k", str(k),
            "--data-dir", data_dir or index_dir(config)]
    return _run(args)["hits"]


def index(paths: list[str], config: str, data_dir: str | None = None) -> dict:
    """Переиндексирует корпус с параметрами нарезки из конфига."""
    args = ["index", "--config", config, "--data-dir", data_dir or index_dir(config)]
    args += ["--paths", *paths]
    return _run(args)


if __name__ == "__main__":
    import sys
    hits = retrieve(sys.argv[1], "evals/configs/vector_only.json")
    for h in hits:
        print(f"{h['score']:+.4f}  {h['chunk_id']}  [{h['char_start']}:{h['char_end']}]  {h['path']}")
