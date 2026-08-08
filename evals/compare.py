"""Парное сравнение двух конфигураций на одном golden set.

    python evals/compare.py chunk_1000 chunk_1000_prefix --golden golden.dev

Средние по 30 запросам легко обманывают. Здесь считается ПОПАРНАЯ разница
по каждому запросу, счёт «улучшилось / ухудшилось / без изменений» и
бутстрэп-интервал на среднюю разницу. Если интервал накрывает ноль —
разницу нельзя называть эффектом, сколько бы ни было в таблице.
"""

import argparse
import json
import random
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def load(name: str, golden: str) -> dict:
    p = REPO / "evals" / "results" / f"{name}__{golden}.json"
    if not p.exists():
        sys.exit(f"нет файла {p}")
    data = json.loads(p.read_text(encoding="utf-8"))
    return {m["id"]: m for m in data["per_query"] if not m["negative"]}


def bootstrap(diffs: list[float], rounds: int = 10000, seed: int = 7) -> tuple[float, float]:
    rng = random.Random(seed)
    n = len(diffs)
    means = []
    for _ in range(rounds):
        means.append(sum(rng.choice(diffs) for _ in range(n)) / n)
    means.sort()
    return means[int(0.025 * rounds)], means[int(0.975 * rounds)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    ap.add_argument("other")
    ap.add_argument("--golden", default="golden.dev")
    ap.add_argument("--metric", default="recall@5")
    args = ap.parse_args()

    a, b = load(args.base, args.golden), load(args.other, args.golden)
    ids = sorted(set(a) & set(b))
    if not ids:
        sys.exit("нет общих запросов")

    diffs = [b[i][args.metric] - a[i][args.metric] for i in ids]
    better = [i for i in ids if b[i][args.metric] > a[i][args.metric]]
    worse = [i for i in ids if b[i][args.metric] < a[i][args.metric]]
    mean = sum(diffs) / len(diffs)
    lo, hi = bootstrap(diffs)

    print(f"\n{args.other} против {args.base}  ·  {args.metric}  ·  {len(ids)} запросов\n")
    print(f"  среднее у {args.base:20} {sum(a[i][args.metric] for i in ids)/len(ids):.3f}")
    print(f"  среднее у {args.other:20} {sum(b[i][args.metric] for i in ids)/len(ids):.3f}")
    print(f"  разница                       {mean:+.4f}   95% бутстрэп [{lo:+.4f}, {hi:+.4f}]")
    print(f"\n  улучшилось на {len(better)} запросах, ухудшилось на {len(worse)}, "
          f"без изменений на {len(ids) - len(better) - len(worse)}")

    # Порог, а не строгий ноль: нижняя граница вида +0.0004 практически
    # неотличима от нуля, и объявлять по ней эффект — самообман.
    eps = 0.005
    if lo > eps:
        print("\n  Интервал не накрывает ноль — улучшение можно называть улучшением.")
    elif lo > 0:
        print("\n  НА ГРАНИ: нижняя граница почти ноль. Направление верное,")
        print("  но заявлять эффект на этом наборе нельзя.")
    elif hi < -eps:
        print("\n  Интервал целиком ниже нуля — стало достоверно хуже.")
    else:
        print("\n  ИНТЕРВАЛ НАКРЫВАЕТ НОЛЬ. На этом наборе разница неотличима от шума —")
        print("  называть это эффектом нельзя, нужен больший golden set.")

    def label(ids_: list[str]) -> str:
        return ", ".join("{} ({})".format(i, b[i]["kind"]) for i in ids_[:8])

    if better:
        print("\n  где стало лучше: " + label(better))
    if worse:
        print("  где стало хуже:  " + label(worse))
    return 0


if __name__ == "__main__":
    sys.exit(main())
