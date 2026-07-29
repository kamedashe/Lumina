"""Корректирующий RAG-граф на LangGraph.

Зачем отдельный процесс: LangGraph — библиотека Python, а агентный цикл Lumina
написан на Rust. Граф запускается как sidecar и общается с хостом построчным
JSON по stdin/stdout.

Принцип разделения: **граф занимается только оркестрацией, весь ввод-вывод
остаётся на стороне Rust**. Когда узлу нужен поиск по векторам или вызов
модели, он отправляет хосту callback и ждёт ответа. Благодаря этому здесь нет
ни ключей API, ни клиентов провайдеров, ни знания о том, какое из трёх
векторных хранилищ сейчас активно — всё это уже реализовано в Rust.

Сам граф решает конкретную проблему одиночного поиска: сырой разговорный
вопрос («какой стек фигурирует в этом резюме?») плохо ложится в эмбеддинг —
половина токенов не несёт смысла. Граф сначала переформулирует вопрос в
поисковый запрос, затем оценивает найденное на релевантность и, если материала
мало, заходит на второй круг с другой формулировкой.

    START → plan → retrieve → grade → (relevant?) → compose → END
                      ↑                    │
                      └─────── refine ←────┘
"""

from __future__ import annotations

import json
import operator
import re
import sys
from typing import Annotated, Any, Literal, TypedDict

from langgraph.graph import END, START, StateGraph

# На Windows Python кодирует stdout/stdin в кодировку локали (cp1251), и любой
# символ вне неё — кириллица в запросе, стрелка в следе графа — валит процесс
# с UnicodeEncodeError. Хост говорит строго в UTF-8, поэтому фиксируем её явно.
sys.stdin.reconfigure(encoding="utf-8", errors="replace")
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

# Сколько заходов на поиск допускаем: первый + одна попытка переформулировать.
MAX_ATTEMPTS = 2
# Сколько релевантных фрагментов считаем достаточным, чтобы не идти на второй круг.
MIN_RELEVANT = 2
# Запрашиваем с запасом: часть фрагментов отсеется на оценке релевантности.
TOP_K = 8
# Ограничение на длину фрагмента в промпте оценщика — чтобы не раздувать запрос.
GRADE_EXCERPT = 600


# --------------------------------------------------------------------------
# Протокол общения с хостом
# --------------------------------------------------------------------------

_call_counter = 0


def _emit(message: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def _call_host(kind: str, payload: dict[str, Any]) -> dict[str, Any]:
    """Просит хост выполнить операцию и блокируется до ответа.

    Синхронный запрос-ответ: граф исполняется последовательно, поэтому
    усложнять асинхронностью нечего.
    """
    global _call_counter
    _call_counter += 1
    call_id = f"c{_call_counter}"

    _emit({"type": "callback", "call_id": call_id, "kind": kind, "payload": payload})

    line = sys.stdin.readline()
    if not line:
        raise RuntimeError("хост закрыл соединение")

    response = json.loads(line)
    if response.get("call_id") != call_id:
        raise RuntimeError(
            f"рассинхронизация протокола: ожидали {call_id}, получили {response.get('call_id')}"
        )
    if not response.get("ok"):
        raise RuntimeError(response.get("error") or "хост вернул ошибку без описания")

    return response.get("data") or {}


def _ask_llm(system: str, prompt: str) -> str:
    return (_call_host("llm", {"system": system, "prompt": prompt}).get("text") or "").strip()


def _retrieve(query: str, scope: list[str], top_k: int) -> list[dict[str, Any]]:
    data = _call_host("retrieve", {"query": query, "scope": scope, "top_k": top_k})
    return data.get("hits") or []


# --------------------------------------------------------------------------
# Состояние графа
# --------------------------------------------------------------------------


class SearchState(TypedDict):
    """Состояние, которое узлы передают друг другу.

    `relevant` и `trace` помечены редьюсером operator.add: у графа есть цикл, и
    результаты второго захода должны накапливаться к первому, а не затирать его.
    """

    question: str
    scope: list[str]
    query: str
    attempts: int
    hits: list[dict[str, Any]]
    relevant: Annotated[list[dict[str, Any]], operator.add]
    trace: Annotated[list[str], operator.add]
    # Без редьюсера: compose записывает готовый контекст один раз, перезаписью.
    context: str


# --------------------------------------------------------------------------
# Узлы
# --------------------------------------------------------------------------

_PLAN_SYSTEM = (
    "Ты помогаешь искать по личным документам пользователя. "
    "Преобразуй вопрос в короткий поисковый запрос: только значимые термины, "
    "без вопросительных слов и вежливых оборотов. Отвечай одной строкой, без пояснений."
)

_REFINE_SYSTEM = (
    "Предыдущий поисковый запрос дал мало релевантного. "
    "Предложи другую формулировку: синонимы, более общие или более конкретные термины. "
    "Отвечай одной строкой, без пояснений."
)

_GRADE_SYSTEM = (
    "Ты оцениваешь релевантность фрагментов документов вопросу пользователя. "
    "Верни только номера подходящих фрагментов через запятую, например: 0, 3, 5. "
    "Если не подходит ни один — верни слово NONE. Ничего больше не пиши."
)


def _first_line(text: str) -> str:
    """Модель нередко добавляет пояснения — берём только первую содержательную строку."""
    for line in text.splitlines():
        cleaned = line.strip().strip('"').strip("«»").strip()
        if cleaned:
            return cleaned
    return ""


def plan(state: SearchState) -> dict[str, Any]:
    """Переформулирует вопрос пользователя в поисковый запрос."""
    question = state["question"]
    try:
        query = _first_line(_ask_llm(_PLAN_SYSTEM, f"Вопрос: {question}")) or question
    except Exception:
        # Переформулировка — улучшение, а не необходимость: при сбое ищем как есть.
        query = question

    return {
        "query": query,
        "attempts": 0,
        "trace": [f"plan → запрос: «{query}»"],
    }


def retrieve(state: SearchState) -> dict[str, Any]:
    """Векторный поиск через хост."""
    hits = _retrieve(state["query"], state["scope"], TOP_K)
    attempt = state["attempts"] + 1
    return {
        "hits": hits,
        "attempts": attempt,
        "trace": [f"retrieve #{attempt} → фрагментов: {len(hits)}"],
    }


def _parse_indices(text: str, limit: int) -> list[int]:
    if "NONE" in text.upper():
        return []
    found = {int(n) for n in re.findall(r"\d+", text)}
    return sorted(i for i in found if 0 <= i < limit)


def grade(state: SearchState) -> dict[str, Any]:
    """Отсеивает фрагменты, которые прошли по косинусу, но по смыслу не подходят.

    Оценка идёт одним запросом на все фрагменты сразу: отдельный вызов на каждый
    был бы вдвое-втрое дороже и медленнее без выигрыша в качестве.
    """
    hits = state["hits"]
    if not hits:
        return {"trace": ["grade → нечего оценивать"]}

    numbered = "\n\n".join(
        f"[{i}] {hit.get('content', '')[:GRADE_EXCERPT]}" for i, hit in enumerate(hits)
    )
    prompt = f"Вопрос: {state['question']}\n\nФрагменты:\n{numbered}"

    try:
        keep = _parse_indices(_ask_llm(_GRADE_SYSTEM, prompt), len(hits))
    except Exception:
        # Оценщик недоступен — лучше отдать всё найденное, чем ничего.
        return {
            "relevant": hits,
            "trace": ["grade → оценщик недоступен, берём всё найденное"],
        }

    relevant = [hits[i] for i in keep]
    return {
        "relevant": relevant,
        "trace": [f"grade → релевантно {len(relevant)} из {len(hits)}"],
    }


def refine(state: SearchState) -> dict[str, Any]:
    """Другая формулировка запроса перед повторным заходом."""
    prompt = (
        f"Исходный вопрос: {state['question']}\n"
        f"Неудачный запрос: {state['query']}\n\n"
        "Предложи другую формулировку."
    )
    try:
        query = _first_line(_ask_llm(_REFINE_SYSTEM, prompt)) or state["question"]
    except Exception:
        query = state["question"]

    return {"query": query, "trace": [f"refine → новый запрос: «{query}»"]}


def compose(state: SearchState) -> dict[str, Any]:
    """Собирает финальный контекст.

    За два захода один и тот же фрагмент мог попасть в выборку дважды —
    дедуплицируем по содержимому, сохраняя порядок (он отражает релевантность).
    Результат пишем в `context`, а не обратно в `relevant`: у последнего
    редьюсер operator.add, и запись туда не заменила бы список, а удвоила его.
    """
    seen: set[str] = set()
    unique: list[str] = []
    for hit in state["relevant"]:
        content = hit.get("content", "")
        if content and content not in seen:
            seen.add(content)
            unique.append(content)

    return {
        "context": "\n---\n".join(unique),
        "trace": [f"compose → итоговых фрагментов: {len(unique)}"],
    }


def decide(state: SearchState) -> Literal["compose", "refine"]:
    """Хватит ли собранного или зайти на второй круг."""
    if len(state["relevant"]) >= MIN_RELEVANT:
        return "compose"
    if state["attempts"] >= MAX_ATTEMPTS:
        return "compose"
    return "refine"


# --------------------------------------------------------------------------
# Сборка графа
# --------------------------------------------------------------------------


def build_graph():
    builder = StateGraph(SearchState)

    builder.add_node("plan", plan)
    builder.add_node("retrieve", retrieve)
    builder.add_node("grade", grade)
    builder.add_node("refine", refine)
    builder.add_node("compose", compose)

    builder.add_edge(START, "plan")
    builder.add_edge("plan", "retrieve")
    builder.add_edge("retrieve", "grade")
    # path_map обязателен: без него LangGraph не знает, куда ведут ветки.
    builder.add_conditional_edges("grade", decide, {"compose": "compose", "refine": "refine"})
    builder.add_edge("refine", "retrieve")  # цикл
    builder.add_edge("compose", END)

    return builder.compile()


GRAPH = build_graph()


def run_search(question: str, scope: list[str]) -> dict[str, Any]:
    result = GRAPH.invoke(
        {
            "question": question,
            "scope": scope,
            "query": question,
            "attempts": 0,
            "hits": [],
            "relevant": [],
            "trace": [],
            "context": "",
        }
    )

    return {"context": result.get("context", ""), "trace": result.get("trace", [])}


# --------------------------------------------------------------------------
# Цикл обработки запросов хоста
# --------------------------------------------------------------------------


def main() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            _emit({"type": "error", "message": f"некорректный JSON: {exc}"})
            continue

        kind = request.get("type")

        if kind == "ping":
            _emit({"type": "pong"})
            continue

        if kind != "search":
            _emit({"type": "error", "message": f"неизвестный тип запроса: {kind}"})
            continue

        try:
            outcome = run_search(request.get("query") or "", request.get("scope") or [])
            _emit({"type": "result", **outcome})
        except Exception as exc:  # noqa: BLE001 — хосту нужен текст любой ошибки
            _emit({"type": "error", "message": f"{type(exc).__name__}: {exc}"})


if __name__ == "__main__":
    main()
