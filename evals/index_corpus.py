"""Индексация корпуса для eval-прогона.

    python evals/index_corpus.py --config evals/configs/vector_only.json

Берёт ровно тот же список файлов, что и annotate.py (CORPUS_GLOBS), поэтому
проиндексированное гарантированно совпадает с тем, что размечено в golden set.
Параметры нарезки берутся из конфига — на этом и держится свип размера чанка.
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from annotate import CORPUS_GLOBS, REPO  # noqa: E402
from retrieve import index  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--data-dir", default=None)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    paths: list[str] = []
    for pattern in CORPUS_GLOBS:
        for p in sorted(REPO.glob(pattern)):
            rel = p.relative_to(REPO).as_posix()
            if p.is_file() and rel not in paths:
                paths.append(rel)

    if not paths:
        sys.exit("Корпус пуст — проверь CORPUS_GLOBS в annotate.py")

    cfg = json.loads(Path(args.config).read_text(encoding="utf-8"))
    print(f"конфиг {cfg.get('name')} · чанк {cfg.get('chunk_size')}/{cfg.get('chunk_overlap')}")
    print(f"файлов: {len(paths)}")
    for p in paths:
        print(f"  {p}")

    # Сверка с golden set: если размечен файл, которого нет в индексе,
    # recall по нему будет нулевым, и это выглядело бы как плохой поиск.
    golden = REPO / "evals" / "golden.jsonl"
    if golden.exists():
        marked = {
            s["path"]
            for line in golden.read_text(encoding="utf-8").splitlines()
            if line.strip()
            for s in json.loads(line).get("relevant_spans", [])
        }
        missing = sorted(marked - set(paths))
        if missing:
            print("\n!! размечены, но НЕ индексируются:")
            for m in missing:
                print(f"   {m}")
            print("   добавь их в CORPUS_GLOBS, иначе recall по ним будет 0")

    if args.dry_run:
        return 0

    print("\nиндексирую (по одному вызову эмбеддингов на чанк, это небыстро)…")
    print(index(paths, args.config, data_dir=args.data_dir))
    return 0


if __name__ == "__main__":
    sys.exit(main())
