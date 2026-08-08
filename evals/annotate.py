"""Интерактивная разметка golden set.

Запуск из корня репозитория:

    python evals/annotate.py              # размечать
    python evals/annotate.py --fix        # починить помеченные строки
    python evals/annotate.py --negatives  # добрать негативов

Скрипт сам показывает кусок документа, сам следит за пропорцией verbatim /
paraphrase / real, сам пишет строку в evals/golden.jsonl и сам проверяет,
что цитата находится в файле ровно один раз.

От тебя нужны только два решения: какой вопрос человек задал бы, чтобы
попасть в этот кусок, и какая строка на него отвечает.
"""

import json
import os
import random
import re
import sys
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

REPO = Path(__file__).resolve().parent.parent
GOLDEN = REPO / "evals" / "golden.jsonl"

# Что считаем корпусом. Правь под себя — это обычный список путей.
CORPUS_GLOBS = [
    "README.md",
    "PLUGINS.md",
    "src-tauri/src/*.rs",
    "src-tauri/src/**/*.rs",
    "sidecar/*.py",
]

TARGET_MIX = {"verbatim": 0.40, "paraphrase": 0.40, "real": 0.20}
WINDOW_CHARS = 900
SEP = "─" * 72


# ─────────────────────────── корпус ───────────────────────────

def load_corpus() -> dict[str, str]:
    docs = {}
    for pattern in CORPUS_GLOBS:
        for p in sorted(REPO.glob(pattern)):
            if p.is_file():
                rel = p.relative_to(REPO).as_posix()
                docs.setdefault(rel, p.read_text(encoding="utf-8", errors="replace"))
    return docs


def fragments(text: str) -> list[str]:
    """Режет текст на кусочки, которые можно выбрать номером.

    Строка — естественная единица и для markdown, и для кода. Слишком
    длинные строки дополнительно делятся по границам предложений.
    """
    out = []
    for line in text.splitlines():
        line = line.rstrip()
        if not line.strip():
            continue
        if len(line) <= 200:
            out.append(line)
        else:
            out.extend(s.strip() for s in re.split(r"(?<=[.!?])\s+", line) if s.strip())
    return out


def windows(frags: list[str]) -> list[tuple[int, int]]:
    """Группирует фрагменты в окна примерно по WINDOW_CHARS символов."""
    spans, start, size = [], 0, 0
    for i, f in enumerate(frags):
        size += len(f) + 1
        if size >= WINDOW_CHARS:
            spans.append((start, i + 1))
            start, size = i + 1, 0
    if start < len(frags):
        spans.append((start, len(frags)))
    return spans


def unique_quote(doc: str, frags: list[str], idx: int) -> str | None:
    """Возвращает цитату, встречающуюся в документе ровно один раз.

    Если фрагмент неуникален (типичное для кода `}` или пустой заголовок),
    приклеивает соседние фрагменты, пока не станет уникальным.
    """
    lo = hi = idx
    for _ in range(4):
        quote = "\n".join(frags[lo : hi + 1]).strip()
        if quote and doc.count(quote) == 1:
            return quote
        if hi + 1 < len(frags):
            hi += 1
        elif lo > 0:
            lo -= 1
        else:
            return None
    return None


# ─────────────────────────── файл разметки ───────────────────────────

def load_rows() -> list[dict]:
    if not GOLDEN.exists():
        return []
    return [json.loads(l) for l in GOLDEN.read_text(encoding="utf-8").splitlines() if l.strip()]


def append_row(row: dict) -> None:
    with GOLDEN.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")


def next_id(rows: list[dict]) -> str:
    nums = [int(r["id"][1:]) for r in rows if re.fullmatch(r"q\d+", r.get("id", ""))]
    return "q%03d" % ((max(nums) + 1) if nums else 1)


def stats(rows: list[dict]) -> tuple[dict, int]:
    pos = [r for r in rows if r.get("relevant_spans")]
    counts = {k: sum(1 for r in pos if r.get("kind") == k) for k in TARGET_MIX}
    return counts, len(rows) - len(pos)


def pick_kind(rows: list[dict]) -> str:
    """Выбирает вид, который сильнее всего отстаёт от целевой пропорции."""
    counts, _ = stats(rows)
    total = sum(counts.values()) or 1
    return min(TARGET_MIX, key=lambda k: counts[k] / total - TARGET_MIX[k])


def show_progress(rows: list[dict]) -> None:
    counts, neg = stats(rows)
    total = sum(counts.values())
    bits = "  ".join(f"{k} {counts[k]}" for k in ("verbatim", "paraphrase", "real"))
    print(f"\n{SEP}\nвсего {total + neg}/50   {bits}   негативов {neg}")


def clear() -> None:
    os.system("cls" if os.name == "nt" else "clear")


def ask(prompt: str) -> str:
    try:
        return input(prompt).strip()
    except (EOFError, KeyboardInterrupt):
        print("\nсохранено, выходим.")
        sys.exit(0)


# ─────────────────────────── режимы ───────────────────────────

def do_passage(rows: list[dict], docs: dict, kind: str) -> dict | None:
    """verbatim / paraphrase: показать кусок → получить вопрос → выбрать строку."""
    path = random.choice(list(docs))
    frags = fragments(docs[path])
    if not frags:
        return None
    lo, hi = random.choice(windows(frags))
    view = frags[lo:hi]

    print(f"\n{SEP}\n{kind.upper()}   ·   {path}\n{SEP}")
    for i, f in enumerate(view, 1):
        print(f"{i:3} │ {f[:160]}")
    print(SEP)

    if kind == "paraphrase":
        ask("прочитал? нажми Enter — текст спрячется, чтобы вопрос вышел своими словами ")
        clear()
        print(f"{SEP}\nPARAPHRASE · {path} · текст скрыт намеренно\n{SEP}")

    query = ask("вопрос (Enter — пропустить кусок): ")
    if not query:
        return None

    if kind == "paraphrase":
        for i, f in enumerate(view, 1):
            print(f"{i:3} │ {f[:160]}")
        print(SEP)

    nums = ask("номера строк с ответом (например 3 или 3,4; 'n' — ответа тут нет): ")
    if nums.lower() == "n":
        return {"id": next_id(rows), "query": query, "relevant_spans": [],
                "kind": kind, "note": "негатив"}

    spans = []
    for raw in nums.replace(" ", "").split(","):
        if not raw.isdigit() or not (1 <= int(raw) <= len(view)):
            print(f"  пропускаю «{raw}» — нет такого номера")
            continue
        quote = unique_quote(docs[path], frags, lo + int(raw) - 1)
        if quote is None:
            print(f"  строка {raw} не даёт уникальной цитаты, пропускаю")
            continue
        spans.append({"path": path, "quote": quote})

    if not spans:
        print("  ни одной цитаты — кусок пропущен")
        return None

    note = ask("заметка (Enter — пусто): ")
    return {"id": next_id(rows), "query": query, "relevant_spans": spans,
            "kind": kind, "note": note}


def do_real(rows: list[dict], docs: dict) -> dict | None:
    """real: сначала твой настоящий вопрос, потом ищем, где на него ответ."""
    print(f"\n{SEP}\nREAL · вспомни вопрос, который ты сам задавал Lumina\n{SEP}")
    query = ask("вопрос (Enter — пропустить): ")
    if not query:
        return None

    while True:
        needle = ask("слово для поиска в корпусе ('n' — ответа в корпусе нет): ")
        if needle.lower() == "n":
            return {"id": next_id(rows), "query": query, "relevant_spans": [],
                    "kind": "real", "note": "негатив"}
        if not needle:
            continue

        found = []
        for path, text in docs.items():
            frags = fragments(text)
            for i, f in enumerate(frags):
                if needle.lower() in f.lower():
                    found.append((path, frags, i, f))
        if not found:
            print("  ничего не нашлось, попробуй другое слово")
            continue

        found = found[:15]
        for n, (path, _, _, f) in enumerate(found, 1):
            print(f"{n:3} │ {path:34} │ {f[:110]}")
        print(SEP)
        nums = ask("номера подходящих (Enter — искать другое слово): ")
        if not nums:
            continue

        spans = []
        for raw in nums.replace(" ", "").split(","):
            if raw.isdigit() and 1 <= int(raw) <= len(found):
                path, frags, i, _ = found[int(raw) - 1]
                quote = unique_quote(docs[path], frags, i)
                if quote:
                    spans.append({"path": path, "quote": quote})
        if not spans:
            print("  ничего не выбрано")
            continue

        note = ask("заметка (Enter — пусто): ")
        return {"id": next_id(rows), "query": query, "relevant_spans": spans,
                "kind": "real", "note": note}




# ─────────────────────────── починка помеченных ───────────────────────────

def do_fix() -> None:
    """Проходит только по строкам, которые помечены как меряющие не то.

    Пишет файл после КАЖДОЙ правки — Ctrl+C ничего не теряет.
    """
    sys.path.insert(0, str(REPO / "evals"))
    from golden_tool import row_flags

    rows = load_rows()
    todo = [r["id"] for r in rows if row_flags(r)]
    if not todo:
        print("Помеченных строк нет — набор чистый.")
        return

    print(f"Помечено {len(todo)} строк из {len(rows)}.\n")
    print("На каждой: впиши НОВЫЙ вопрос, или 'd' чтобы удалить строку,")
    print("или Enter чтобы оставить как есть. Ctrl+C — выход, всё сохранено.\n")

    changed = deleted = 0

    for n, rid in enumerate(todo, 1):
        row = next((r for r in rows if r["id"] == rid), None)
        if row is None:
            continue

        print(f"\n{SEP}\n[{n}/{len(todo)}]  {rid}  ·  вид: {row.get('kind')}")
        for f in row_flags(row):
            print(f"  проблема: {f}")
        print(f"{SEP}\nсейчас вопрос: {row.get('query', '')!r}\n")
        print("ответ, на который он должен наводить:")
        for sp in row.get("relevant_spans", []):
            print(f"  {sp['path']}")
            for line in sp["quote"].splitlines():
                print(f"    | {line[:150]}")
        if not row.get("relevant_spans"):
            print("  (негатив — спанов нет)")
        print(SEP)

        answer = ask("новый вопрос ('d' удалить, Enter оставить): ")
        if not answer:
            continue

        if answer.lower() == "d":
            rows = [r for r in rows if r["id"] != rid]
            deleted += 1
            save_rows(rows)
            print("  x удалена")
            continue

        row["query"] = answer
        kind = ask(f"вид — v/p/r (Enter оставить '{row.get('kind')}'): ").lower()
        row["kind"] = {"v": "verbatim", "p": "paraphrase", "r": "real"}.get(kind, row.get("kind"))
        changed += 1
        save_rows(rows)
        print(f"  v {rid} обновлён")

    print(f"\n{SEP}\nисправлено {changed}, удалено {deleted}, осталось строк {len(rows)}")
    print("проверь:  python evals/golden_tool.py review evals/golden.jsonl")


def do_negatives() -> None:
    """Добор негативов: вопрос, ответа на который в корпусе нет."""
    rows = load_rows()
    neg = sum(1 for r in rows if not r.get("relevant_spans"))
    print(f"Негативов сейчас {neg}, план 5–10.")
    print("Правдоподобный вопрос, ответа на который в корпусе НЕТ. Пустая строка — выход.\n")
    while True:
        q = ask("вопрос-негатив: ")
        if not q:
            break
        row = {"id": next_id(rows), "query": q, "relevant_spans": [],
               "kind": "real", "note": "негатив"}
        append_row(row)
        rows.append(row)
        neg += 1
        print(f"  ✓ {row['id']}   негативов теперь {neg}")


def save_rows(rows: list[dict]) -> None:
    GOLDEN.write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    if "--fix" in sys.argv:
        do_fix()
        return
    if "--negatives" in sys.argv:
        do_negatives()
        return

    docs = load_corpus()
    if not docs:
        sys.exit("Корпус пуст — проверь CORPUS_GLOBS в начале файла.")
    rows = load_rows()

    print(f"Корпус: {len(docs)} файлов, {sum(len(t) for t in docs.values()) // 1000} тыс. символов")
    print("Ctrl+C в любой момент — всё уже записанное сохранено.\n")

    while True:
        show_progress(rows)
        kind = pick_kind(rows)
        row = do_real(rows, docs) if kind == "real" else do_passage(rows, docs, kind)
        if row:
            append_row(row)
            rows.append(row)
            print(f"  ✓ записано {row['id']}")


if __name__ == "__main__":
    main()
