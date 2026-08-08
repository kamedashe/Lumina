"""Прогон одной конфигурации по golden set.

    python evals/run_eval.py --golden evals/golden.dev.jsonl \
                             --config evals/configs/vector_only.json

Пишет результат в evals/results/<config>__<golden>.json и трассу каждого
запроса в evals/results/<...>.trace.jsonl.
"""

import argparse
import hashlib
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from metrics import evaluate  # noqa: E402
from retrieve import retrieve as lumina_retrieve  # noqa: E402

REPO = Path(__file__).resolve().parent.parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--golden", required=True)
    ap.add_argument("--config", required=True)
    ap.add_argument("--k", type=int, default=20)
    ap.add_argument("--data-dir", default=None)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    golden_path = Path(args.golden)
    rows = [json.loads(l) for l in golden_path.read_text(encoding="utf-8").splitlines() if l.strip()]
    config_text = Path(args.config).read_text(encoding="utf-8")
    config_hash = hashlib.sha256(config_text.encode()).hexdigest()[:8]
    config_name = json.loads(config_text).get("name", Path(args.config).stem)

    out = Path(args.out) if args.out else REPO / "evals" / "results" / f"{config_name}__{golden_path.stem}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    trace = out.with_suffix(".trace.jsonl")
    trace_file = trace.open("w", encoding="utf-8")

    print(f"конфиг {config_name} ({config_hash}) · {golden_path.name} · {len(rows)} запросов")

    def retriever(query: str) -> list[dict]:
        t0 = time.perf_counter()
        hits = lumina_retrieve(query, args.config, k=args.k, data_dir=args.data_dir)
        latency_ms = round((time.perf_counter() - t0) * 1000)
        trace_file.write(json.dumps({
            "ts": datetime.now(timezone.utc).isoformat(),
            "query": query,
            "config_hash": config_hash,
            "latency_ms": latency_ms,
            "hits": [{"chunk_id": h["chunk_id"], "score": h["score"]} for h in hits],
        }, ensure_ascii=False) + "\n")
        print(".", end="", flush=True)
        return hits

    summary, per_query = evaluate(rows, retriever)
    trace_file.close()
    print()

    payload = {
        "config": config_name,
        "config_hash": config_hash,
        "config_file": args.config,
        "golden": golden_path.name,
        "k": args.k,
        "ran_at": datetime.now(timezone.utc).isoformat(),
        "summary": summary,
        "per_query": per_query,
    }
    out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    _print_table(summary)
    print(f"\nрезультат: {out}\nтрасса:    {trace}")
    return 0


def _fmt(v) -> str:
    return "  —  " if v is None else f"{v:.3f}"


def _print_table(s: dict) -> None:
    print(f"\nзапросов {s['queries']}, негативов {s['negatives']}")
    cols = ["recall@1", "recall@5", "recall@10", "ndcg@5", "ndcg@10", "mrr"]
    head = "  ".join(f"{c:>9}" for c in cols)
    print(f"\n{'':14}{head}")
    print(f"{'ВСЕГО':14}" + "  ".join(f"{_fmt(s[c]):>9}" for c in cols))
    for kind, vals in s["by_kind"].items():
        label = f"{kind} ({vals['n']})"
        print(f"{label:14}" + "  ".join(f"{_fmt(vals[c]):>9}" for c in cols))
    print("\nразрыв verbatim vs paraphrase по recall@5 — это вклад семантики поверх слов.")


if __name__ == "__main__":
    sys.exit(main())
