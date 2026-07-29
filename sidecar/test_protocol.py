"""Прогон графа как настоящего подпроцесса — проверка протокола, а не логики.

test_graph.py подменяет _call_host и проверяет решения графа. Здесь наоборот:
graph.py запускается ровно так, как его запускает Rust (`python -u graph.py`),
и общение идёт через реальные stdin/stdout. Это ловит то, что подменой не
поймать — буферизацию, разбиение на строки, рассинхронизацию call_id.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).with_name("graph.py")

# Консоль Windows по умолчанию cp1251 — вывод теста в ней бы падал.
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

CHUNKS = [
    {"content": "Стек: Rust, Tauri v2, React 19, TypeScript.", "score": 0.91},
    {"content": "Рецепт борща: свёкла, капуста, картофель.", "score": 0.55},
]


def serve(process: subprocess.Popen, request: dict) -> dict:
    """Играет роль Rust-хоста: шлёт запрос и обслуживает callback'и до результата."""
    process.stdin.write(json.dumps(request, ensure_ascii=False) + "\n")
    process.stdin.flush()

    seen_kinds: list[str] = []

    while True:
        line = process.stdout.readline()
        if not line:
            stderr = process.stderr.read()
            raise AssertionError(f"граф молча завершился. stderr:\n{stderr}")

        message = json.loads(line)
        kind = message.get("type")

        if kind == "callback":
            seen_kinds.append(message["kind"])
            payload = message["payload"]

            if message["kind"] == "retrieve":
                data = {"hits": CHUNKS}
            elif message["kind"] == "llm":
                # Оценщик получает пронумерованные фрагменты — оставляем только первый.
                data = {"text": "0"} if "релевантность" in payload["system"] else {"text": "стек резюме"}
            else:
                raise AssertionError(f"неизвестный callback: {message['kind']}")

            process.stdin.write(
                json.dumps({"call_id": message["call_id"], "ok": True, "data": data}) + "\n"
            )
            process.stdin.flush()
            continue

        if kind in ("result", "error", "pong"):
            message["_seen_kinds"] = seen_kinds
            return message

        raise AssertionError(f"неожиданное сообщение: {message}")


def main() -> int:
    process = subprocess.Popen(
        [sys.executable, "-u", str(SCRIPT)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )

    failures = 0
    try:
        # 1. ping/pong — хост проверяет живость процесса
        pong = serve(process, {"type": "ping"})
        if pong.get("type") == "pong":
            print("  OK    ping → pong")
        else:
            failures += 1
            print(f"  FAIL  ping: получили {pong}")

        # 2. Полный прогон поиска
        result = serve(process, {"type": "search", "query": "какой стек в резюме?", "scope": []})

        if result.get("type") != "result":
            failures += 1
            print(f"  FAIL  search: {result}")
        else:
            context = result["context"]
            checks = [
                ("релевантное попало в контекст", "Rust, Tauri" in context),
                ("нерелевантное отсеяно", "борщ" not in context),
                ("вернулся след графа", len(result.get("trace", [])) >= 4),
                ("были вызовы retrieve", "retrieve" in result["_seen_kinds"]),
                ("были вызовы llm", "llm" in result["_seen_kinds"]),
            ]
            for name, passed in checks:
                if passed:
                    print(f"  OK    {name}")
                else:
                    failures += 1
                    print(f"  FAIL  {name}")

            print("\n  След графа:")
            for step in result.get("trace", []):
                print(f"    · {step}")

        # 3. Некорректный запрос не должен убивать процесс
        bad = serve(process, {"type": "нет-такого"})
        if bad.get("type") == "error":
            print("\n  OK    неизвестный тип запроса → ошибка без падения")
        else:
            failures += 1
            print(f"\n  FAIL  неизвестный тип: {bad}")

    finally:
        process.stdin.close()
        process.terminate()

    print(f"\n{'все проверки прошли' if not failures else f'провалено: {failures}'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
