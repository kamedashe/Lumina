"""Блок 3: прогон нескольких конфигураций и сравнительная таблица.

    python evals/sweep.py --golden evals/golden.dev.jsonl

Для каждой конфигурации: свой индекс в evals/index/<name>, переиндексация,
прогон по golden set, строка в общей таблице. Индексы разведены намеренно —
иначе конфигурации затирали бы друг друга и сравнивать было бы нечего.

ТОЛЬКО ПО DEV. Holdout открывается один раз, в самом конце, отдельной командой.
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT = ["chunk_1000", "chunk_1000_prefix", "chunk_250", "chunk_250_prefix"]


def run(cmd: list[str]) -> None:
    proc = subprocess.run([sys.executable] + cmd, cwd=REPO)
    if proc.returncode != 0:
        raise SystemExit(f"упало: {' '.join(cmd)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--golden", default="evals/golden.dev.jsonl")
    ap.add_argument("--configs", nargs="*", default=DEFAULT)
    ap.add_argument("--skip-index", action="store_true", help="индексы уже построены")
    args = ap.parse_args()

    if "holdout" in args.golden:
        print("!! Это holdout. Он открывается ОДИН раз, в самом конце.")
        if input("точно? напиши 'да': ").strip().lower() != "да":
            return 1

    rows = []
    for name in args.configs:
        cfg = f"evals/configs/{name}.json"
        print(f"\n{'=' * 72}\n{name}\n{'=' * 72}")
        t0 = time.perf_counter()

        if not args.skip_index:
            run(["evals/index_corpus.py", "--config", cfg])
        run(["evals/run_eval.py", "--golden", args.golden, "--config", cfg])

        result = REPO / "evals" / "results" / f"{name}__{Path(args.golden).stem}.json"
        summary = json.loads(result.read_text(encoding="utf-8"))["summary"]
        rows.append((name, summary, round(time.perf_counter() - t0)))

    print(f"\n\n{'=' * 96}\nСВОДКА ПО DEV\n{'=' * 96}")
    cols = ["recall@1", "recall@5", "recall@10", "ndcg@10", "mrr"]
    print(f"{'конфигурация':22}" + "".join(f"{c:>11}" for c in cols) + f"{'verbatim@5':>12}{'paraphr@5':>11}{'real@5':>9}")
    for name, s, _ in rows:
        by = s.get("by_kind", {})
        def kk(kind):
            v = by.get(kind, {}).get("recall@5")
            return "   —  " if v is None else f"{v:.3f}"
        print(f"{name:22}" + "".join(f"{(s[c] if s[c] is not None else 0):>11.3f}" for c in cols)
              + f"{kk('verbatim'):>12}{kk('paraphrase'):>11}{kk('real'):>9}")

    best = max(rows, key=lambda r: r[1]["recall@5"] or 0)
    base = next((r for r in rows if r[0] == "chunk_1000"), None)
    print(f"\nлучшая по recall@5: {best[0]} = {best[1]['recall@5']:.3f}")
    if base and base[0] != best[0]:
        delta = (best[1]["recall@5"] or 0) - (base[1]["recall@5"] or 0)
        print(f"против базовой chunk_1000 ({base[1]['recall@5']:.3f}): {delta:+.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
