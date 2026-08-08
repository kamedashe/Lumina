"""Метрики качества поиска. Никаких фреймворков, ~130 строк.

Релевантность определяется ПЕРЕСЕЧЕНИЕМ СПАНОВ, а не совпадением chunk_id:
в golden set размечены цитаты, у хита есть [char_start, char_end], и хит
считается релевантным, если диапазоны пересекаются. Благодаря этому разметка
переживает смену размера чанка — иначе свип чанкинга обнулял бы её.
"""

import math
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
_DOC_CACHE: dict[str, str] = {}


def _doc(path: str) -> str:
    if path not in _DOC_CACHE:
        p = Path(path)
        if not p.is_absolute():
            p = REPO / path
        _DOC_CACHE[path] = p.read_text(encoding="utf-8", errors="replace")
    return _DOC_CACHE[path]


def resolve_spans(row: dict) -> list[tuple[str, int, int]]:
    """Цитата → (path, char_start, char_end). Смещения в СИМВОЛАХ."""
    spans = []
    for s in row.get("relevant_spans", []):
        text = _doc(s["path"])
        start = text.find(s["quote"])
        if start < 0:
            raise ValueError(f"{row['id']}: цитата не найдена в {s['path']}")
        spans.append((s["path"], start, start + len(s["quote"])))
    return spans


def hit_covers(hit: dict, span: tuple[str, int, int]) -> bool:
    path, start, end = span
    return (
        Path(hit["path"]).as_posix().endswith(Path(path).as_posix())
        and hit["char_start"] < end
        and start < hit["char_end"]
    )


def recall_at_k(hits: list[dict], spans: list, k: int) -> float | None:
    """Доля размеченных спанов, покрытых хотя бы одним хитом из топ-k."""
    if not spans:
        return None
    top = hits[:k]
    return sum(1 for s in spans if any(hit_covers(h, s) for h in top)) / len(spans)


def reciprocal_rank(hits: list[dict], spans: list) -> float | None:
    if not spans:
        return None
    for i, h in enumerate(hits, start=1):
        if any(hit_covers(h, s) for s in spans):
            return 1.0 / i
    return 0.0


def ndcg_at_k(hits: list[dict], spans: list, k: int) -> float | None:
    if not spans:
        return None
    dcg = sum(
        1.0 / math.log2(i + 1)
        for i, h in enumerate(hits[:k], start=1)
        if any(hit_covers(h, s) for s in spans)
    )
    idcg = sum(1.0 / math.log2(i + 1) for i in range(1, min(k, len(spans)) + 1))
    return dcg / idcg if idcg else 0.0


def evaluate(rows: list[dict], retrieve, ks=(1, 3, 5, 10, 20)) -> tuple[dict, list[dict]]:
    """rows: golden set; retrieve: query -> ранжированный список хитов.

    Возвращает (сводка, построчные метрики). Построчные обязательны:
    среднее прячет, на каких именно запросах поиск ломается.
    """
    per_query: list[dict] = []

    for row in rows:
        spans = resolve_spans(row)
        hits = retrieve(row["query"])
        m = {
            "id": row["id"],
            "kind": row.get("kind"),
            "query": row["query"],
            "negative": not spans,
            "hits_returned": len(hits),
        }
        if spans:
            for k in ks:
                m[f"recall@{k}"] = recall_at_k(hits, spans, k)
                m[f"ndcg@{k}"] = ndcg_at_k(hits, spans, k)
            m["mrr"] = reciprocal_rank(hits, spans)
        else:
            # У негатива нет правильного ответа — меряем не попадание,
            # а насколько уверенно система выдаёт мусор.
            m["top_score"] = hits[0]["score"] if hits else None
        per_query.append(m)

    positives = [m for m in per_query if not m["negative"]]
    keys = [f"recall@{k}" for k in ks] + [f"ndcg@{k}" for k in ks] + ["mrr"]
    summary = {k: _mean(m[k] for m in positives) for k in keys}
    summary["queries"] = len(positives)
    summary["negatives"] = len(per_query) - len(positives)

    # Разрез по kind: разрыв verbatim vs paraphrase — это и есть
    # измеренный вклад семантики поверх совпадения слов.
    summary["by_kind"] = {}
    for kind in sorted({m["kind"] for m in positives}):
        group = [m for m in positives if m["kind"] == kind]
        summary["by_kind"][kind] = {"n": len(group)} | {
            k: _mean(m[k] for m in group) for k in keys
        }

    return summary, per_query


def _mean(values) -> float | None:
    vals = [v for v in values if v is not None]
    return sum(vals) / len(vals) if vals else None
