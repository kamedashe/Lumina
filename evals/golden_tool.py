"""Проверка и разбиение golden set.

Две команды:

    python evals/golden_tool.py validate evals/golden.jsonl
    python evals/golden_tool.py review   evals/golden.jsonl
    python evals/golden_tool.py split    evals/golden.jsonl

`validate` ловит опечатки в цитатах СРАЗУ, а не на прогоне эвала: каждая
цитата должна находиться в файле ровно один раз, иначе спан не определён.

`split` режет на dev (2/3) и holdout (1/3) со стратификацией по `kind`.
Запускать ОДИН раз, до любого тюнинга. Holdout после этого не открывать.
"""

import json
import random
import sys
from collections import Counter
from pathlib import Path

KINDS = {"verbatim", "paraphrase", "real"}
SEED = 20260808  # фиксированный: разбиение должно воспроизводиться


def load(path: Path) -> list[dict]:
    rows = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as e:
            sys.exit(f"строка {n}: битый JSON — {e}")
    return rows


def read_doc(path: str, cache: dict) -> str:
    if path not in cache:
        p = Path(path)
        if not p.exists():
            cache[path] = None
        elif p.suffix.lower() == ".pdf":
            cache[path] = "__PDF__"
        else:
            cache[path] = p.read_text(encoding="utf-8", errors="replace")
    return cache[path]


def validate(path: Path) -> int:
    rows = load(path)
    cache: dict = {}
    errors, warnings = [], []
    ids = Counter(r.get("id", "") for r in rows)

    for dup, n in ids.items():
        if n > 1:
            errors.append(f"дубль id: {dup} ({n} раз)")

    for r in rows:
        rid = r.get("id", "?")
        if not r.get("query", "").strip():
            errors.append(f"{rid}: пустой query")
        kind = r.get("kind")
        spans = r.get("relevant_spans")
        if spans is None:
            errors.append(f"{rid}: нет поля relevant_spans")
            continue
        if spans and kind not in KINDS:
            errors.append(f"{rid}: kind={kind!r}, ожидается один из {sorted(KINDS)}")

        for s in spans:
            doc = read_doc(s.get("path", ""), cache)
            quote = s.get("quote", "")
            if doc is None:
                errors.append(f"{rid}: файл не найден — {s.get('path')}")
                continue
            if doc == "__PDF__":
                warnings.append(
                    f"{rid}: PDF ({s.get('path')}) — питоновское извлечение "
                    f"не совпадёт с pdf_extract, спан проверить нельзя"
                )
                continue
            if not quote.strip():
                errors.append(f"{rid}: пустая цитата")
                continue
            n = doc.count(quote)
            if n == 0:
                errors.append(f"{rid}: цитата не найдена в {Path(s['path']).name}: {quote[:60]!r}")
            elif n > 1:
                errors.append(f"{rid}: цитата встречается {n} раз — спан неоднозначен: {quote[:60]!r}")
            else:
                s["_char_start"] = doc.index(quote)
                s["_char_end"] = s["_char_start"] + len(quote)

    positives = [r for r in rows if r.get("relevant_spans")]
    negatives = [r for r in rows if not r.get("relevant_spans")]
    by_kind = Counter(r.get("kind") for r in positives)
    total = len(positives) or 1

    print(f"\nстрок: {len(rows)}  (позитивов {len(positives)}, негативов {len(negatives)})")
    for k in sorted(KINDS):
        print(f"  {k:11} {by_kind.get(k, 0):3}  ({by_kind.get(k, 0) / total:.0%})")
    print(f"  спанов всего: {sum(len(r['relevant_spans']) for r in positives)}")
    print(f"  документов:   {len({s['path'] for r in positives for s in r['relevant_spans']})}")

    for w in warnings:
        print("  ⚠ " + w)
    for e in errors:
        print("  ✗ " + e)

    if not errors:
        print("\nОШИБОК НЕТ — можно резать на dev/holdout.")
    else:
        print(f"\n{len(errors)} ошибок — почини до разбиения.")

    if len(rows) < 50:
        print(f"подсказка: строк {len(rows)}, план — 50–80.")
    if len(negatives) < 5:
        print(f"подсказка: негативов {len(negatives)}, план — 5–10.")

    return 1 if errors else 0


def split(path: Path) -> int:
    rows = load(path)
    dev_path = path.with_name("golden.dev.jsonl")
    hold_path = path.with_name("golden.holdout.jsonl")
    if dev_path.exists() or hold_path.exists():
        sys.exit("dev/holdout уже существуют — разбиение делается ОДИН раз. Удали вручную, если правда надо.")

    rng = random.Random(SEED)
    dev, hold = [], []
    # Стратификация: доля holdout одинакова в каждом kind и в негативах.
    groups: dict = {}
    for r in rows:
        key = r.get("kind") if r.get("relevant_spans") else "__negative__"
        groups.setdefault(key, []).append(r)

    for key, group in sorted(groups.items()):
        rng.shuffle(group)
        cut = round(len(group) * 2 / 3)
        dev.extend(group[:cut])
        hold.extend(group[cut:])
        print(f"  {key:12} dev {cut:3} / holdout {len(group) - cut:3}")

    rng.shuffle(dev)
    rng.shuffle(hold)
    for target, data in ((dev_path, dev), (hold_path, hold)):
        target.write_text(
            "\n".join(json.dumps(r, ensure_ascii=False) for r in data) + "\n",
            encoding="utf-8",
        )

    print(f"\ndev:     {dev_path}  ({len(dev)})")
    print(f"holdout: {hold_path}  ({len(hold)})")
    print("\nHoldout открывается ОДИН раз, в самом конце. Не смотри в него сейчас.")
    return 0




# ─────────────────────────── аудит качества ───────────────────────────

def row_flags(row: dict) -> list[str]:
    """Помечает строки, которые померяют не то, что задумано."""
    import re as _re
    f = []
    q = row.get("query", "").strip()
    words = [w for w in _re.split(r"\s+", q) if w]
    core = q.rstrip("?").strip().lower()

    if len(words) < 3:
        f.append("вопрос короче 3 слов — так не спрашивают")
    if _re.search(r"[{};()]|::|->|=>|\bpub \b|\bfn \b|\bconst \b", q):
        f.append("вопрос выглядит как код, а не как вопрос")

    for s in row.get("relevant_spans", []):
        quote = s["quote"].lower()
        if core and core in quote:
            f.append("вопрос дословно содержится в цитате — меряется совпадение строк, не поиск")
            break
        if quote.rstrip(".").strip() in core:
            f.append("цитата содержится в вопросе")
            break
    return f


def review(path: Path) -> int:
    rows = load(path)
    dups = Counter(r.get("query", "").strip().lower() for r in rows)
    flagged = [(r, row_flags(r)) for r in rows]
    flagged = [(r, f) for r, f in flagged if f]

    for r, f in flagged:
        print(f"\n{r['id']} [{r.get('kind')}] {r.get('query', '')[:70]!r}")
        for line in f:
            print(f"      → {line}")
        for s in r.get("relevant_spans", [])[:2]:
            print(f"      цитата: {s['quote'][:78]!r}")

    repeats = [q for q, n in dups.items() if n > 1 and q]
    if repeats:
        print("\nдубликаты вопросов:")
        for q in repeats:
            print(f"      {q[:70]!r}")

    print(f"\nвсего {len(rows)}, помечено {len(flagged)}, чистых {len(rows) - len(flagged)}")
    if flagged:
        print("почини их:  python evals/annotate.py --fix")
    return 0


if __name__ == "__main__":
    CMDS = {"validate": validate, "split": split, "review": review}
    if len(sys.argv) < 3 or sys.argv[1] not in CMDS:
        sys.exit(__doc__)
    sys.exit(CMDS[sys.argv[1]](Path(sys.argv[2])))
