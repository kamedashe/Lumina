"""Прогон графа с подменённым хостом — без Rust, векторного хранилища и LLM.

Проверяет то, ради чего граф вообще существует: цикл повторного поиска,
накопление результатов редьюсером и отсев нерелевантного.

    python -m pytest test_graph.py          (или просто python test_graph.py)
"""

from __future__ import annotations

import graph


class FakeHost:
    """Подменяет _call_host: записывает вызовы и отдаёт заготовленные ответы."""

    def __init__(self, hits_by_attempt: list[list[dict]], grades: list[str]):
        self.hits_by_attempt = hits_by_attempt
        self.grades = grades
        self.retrieve_calls = 0
        self.llm_calls = 0
        self.queries: list[str] = []

    def __call__(self, kind: str, payload: dict):
        if kind == "retrieve":
            self.queries.append(payload["query"])
            index = min(self.retrieve_calls, len(self.hits_by_attempt) - 1)
            self.retrieve_calls += 1
            return {"hits": self.hits_by_attempt[index]}

        if kind == "llm":
            self.llm_calls += 1
            system = payload["system"]
            if "релевантность" in system:
                grade = self.grades[min(len(self.grades) - 1, self.retrieve_calls - 1)]
                return {"text": grade}
            # plan / refine
            return {"text": f"переформулировка-{self.llm_calls}"}

        raise AssertionError(f"неожиданный callback: {kind}")


def chunk(text: str) -> dict:
    return {"content": text, "path": "doc.txt", "score": 0.9}


def run(host: FakeHost, question: str = "какой стек в резюме?"):
    graph._call_host = host  # noqa: SLF001 — подмена ради изоляции теста
    return graph.run_search(question, [])


def test_stops_when_enough_relevant():
    """Достаточно релевантного с первого захода — второго круга быть не должно."""
    host = FakeHost(
        hits_by_attempt=[[chunk("Rust и Tauri"), chunk("Django и PostgreSQL")]],
        grades=["0, 1"],
    )
    result = run(host)

    assert host.retrieve_calls == 1, "лишний заход на поиск"
    assert "Rust и Tauri" in result["context"]
    assert "Django и PostgreSQL" in result["context"]


def test_retries_when_nothing_relevant():
    """Ничего не подошло — граф обязан переформулировать и зайти снова."""
    host = FakeHost(
        hits_by_attempt=[
            [chunk("рецепт борща"), chunk("прогноз погоды")],
            [chunk("стек: Rust, Python"), chunk("опыт с Docker")],
        ],
        grades=["NONE", "0, 1"],
    )
    result = run(host)

    assert host.retrieve_calls == 2, "цикл не сработал"
    assert host.queries[0] != host.queries[1], "запрос не переформулирован"
    assert "рецепт борща" not in result["context"], "нерелевантное просочилось"
    assert "стек: Rust, Python" in result["context"]


def test_respects_attempt_limit():
    """Даже когда релевантного нет вовсе, граф обязан остановиться."""
    host = FakeHost(
        hits_by_attempt=[[chunk("шум")], [chunk("ещё шум")]],
        grades=["NONE", "NONE"],
    )
    result = run(host)

    assert host.retrieve_calls == graph.MAX_ATTEMPTS, "граф не остановился на лимите"
    assert result["context"] == ""


def test_accumulates_across_attempts():
    """Редьюсер operator.add должен сложить находки обоих заходов."""
    host = FakeHost(
        hits_by_attempt=[
            [chunk("частично подходит")],
            [chunk("тоже подходит"), chunk("и это")],
        ],
        grades=["0", "0, 1"],
    )
    result = run(host)

    assert host.retrieve_calls == 2, "первый заход дал 1 < MIN_RELEVANT, нужен второй"
    for expected in ("частично подходит", "тоже подходит", "и это"):
        assert expected in result["context"], f"потеряно: {expected}"


def test_deduplicates_repeated_chunk():
    """Один и тот же фрагмент, найденный дважды, не должен задваиваться."""
    host = FakeHost(
        hits_by_attempt=[[chunk("повтор")], [chunk("повтор"), chunk("новое")]],
        grades=["0", "0, 1"],
    )
    result = run(host)

    assert result["context"].count("повтор") == 1, "дубликат не отсеян"


if __name__ == "__main__":
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    failed = 0
    for test in tests:
        try:
            test()
            print(f"  OK    {test.__name__}")
        except AssertionError as exc:
            failed += 1
            print(f"  FAIL  {test.__name__}: {exc}")
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"  ERROR {test.__name__}: {type(exc).__name__}: {exc}")

    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    raise SystemExit(1 if failed else 0)
