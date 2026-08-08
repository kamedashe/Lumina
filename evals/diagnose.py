"""Диагностика: конвейер сломан или запросы плохие?

    python evals/diagnose.py selftest --config evals/configs/vector_only.json
    python evals/diagnose.py show --id q033 --config evals/configs/vector_only.json

selftest подаёт САМУ ЦИТАТУ как запрос. Если поиск по дословному тексту
ответа не находит содержащий его чанк — сломан конвейер (индексация,
эмбеддинги, спаны). Если находит — конвейер здоров, и низкие метрики
означают, что дело в запросах или в модели эмбеддингов.

show печатает рядом: что должно было найтись и что реально вернулось.
"""

import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from metrics import hit_covers, resolve_spans  # noqa: E402
from retrieve import retrieve  # noqa: E402

REPO = Path(__file__).resolve().parent.parent


def load(path: str) -> list[dict]:
    return [json.loads(l) for l in Path(path).read_text(encoding="utf-8").splitlines() if l.strip()]


def selftest(args) -> int:
    rows = [r for r in load(args.golden) if r.get("relevant_spans")]
    random.Random(1).shuffle(rows)
    rows = rows[: args.n]

    at1 = at5 = at20 = 0
    print(f"Запрос = дословная цитата. {len(rows)} проб.\n")

    for r in rows:
        spans = resolve_spans(r)
        span = spans[0]
        quote = r["relevant_spans"][0]["quote"]
        hits = retrieve(quote[:300], args.config, k=20, data_dir=args.data_dir)

        rank = next((i for i, h in enumerate(hits, 1) if hit_covers(h, span)), None)
        at1 += rank == 1
        at5 += bool(rank and rank <= 5)
        at20 += bool(rank)

        mark = "OK  " if rank == 1 else ("ok  " if rank and rank <= 5 else "MISS")
        print(f"{mark} ранг={rank if rank else '—':>4}  {r['id']}  {Path(span[0]).name}  {quote[:56]!r}")
        if not rank and hits:
            h = hits[0]
            print(f"        вместо этого топ-1: {Path(h['path']).name} [{h['char_start']}:{h['char_end']}] score={h['score']:.4f}")

    n = len(rows) or 1
    cfg = json.loads(Path(args.config).read_text(encoding="utf-8"))
    size = cfg.get("chunk_size", 1000)
    quotes = [len(r["relevant_spans"][0]["quote"]) for r in rows]
    share = (sum(quotes) / len(quotes)) / size

    print(f"\nсамопроверка: rank1 {at1/n:.2f}   top5 {at5/n:.2f}   top20 {at20/n:.2f}")
    print(f"цитата занимает в среднем {share:.1%} чанка ({size} символов)")

    if at20 / n < 0.5:
        print("\nКОНВЕЙЕР ПОД ПОДОЗРЕНИЕМ: дословный текст ответа не находится даже")
        print("в топ-20. Это уже не про формулировки — проверь индексацию и спаны.")
    else:
        print("\nКонвейер работает: чанк с ответом находится, но не всегда высоко.")
        print("При такой доле цитаты в чанке это ожидаемо — запрос из одной строки")
        print("конкурирует с эмбеддингом всего куска. Ручка здесь — размер чанка.")
    return 0


def show(args) -> int:
    rows = load(args.golden)
    row = next((r for r in rows if r["id"] == args.id), None)
    if row is None:
        sys.exit(f"нет строки {args.id} в {args.golden}")

    spans = resolve_spans(row)
    print(f"\n{row['id']} [{row.get('kind')}]  запрос: {row['query']!r}\n")
    print("ДОЛЖНО НАЙТИСЬ:")
    for (path, s, e), sp in zip(spans, row["relevant_spans"]):
        print(f"  {path} [{s}:{e}]")
        print(f"    | {sp['quote'][:150]}")

    hits = retrieve(row["query"], args.config, k=args.k, data_dir=args.data_dir)
    print(f"\nВЕРНУЛОСЬ (топ-{min(args.k, len(hits))}):")
    for i, h in enumerate(hits[: args.k], 1):
        hit = "  <-- ПОПАДАНИЕ" if any(hit_covers(h, s) for s in spans) else ""
        head = " ".join((h.get("content") or "").split())[:90]
        print(f"  {i:2}. {h['score']:+.4f}  {Path(h['path']).name:24} [{h['char_start']}:{h['char_end']}]{hit}")
        if head:
            print(f"        {head}")
    return 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)

    st = sub.add_parser("selftest")
    st.add_argument("--golden", default="evals/golden.dev.jsonl")
    st.add_argument("--config", required=True)
    st.add_argument("--n", type=int, default=12)
    st.add_argument("--data-dir", default=None)
    st.set_defaults(fn=selftest)

    sh = sub.add_parser("show")
    sh.add_argument("--id", required=True)
    sh.add_argument("--golden", default="evals/golden.dev.jsonl")
    sh.add_argument("--config", required=True)
    sh.add_argument("--k", type=int, default=10)
    sh.add_argument("--data-dir", default=None)
    sh.set_defaults(fn=show)

    a = ap.parse_args()
    sys.exit(a.fn(a))
