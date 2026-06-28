#!/usr/bin/env python3

import json
import re
from collections import defaultdict
import sys
from typing import Any


TERM_RE = re.compile(r'Term=Term\(field=([a-zA-Z]+), type=\w+, "([^"]+)"\)')
BOOST_RE = re.compile(r"Boost x([0-9.]+) of")


def find_metric(node: dict, description_prefix: str):
    """
    Find a nested node by the beginning of its description.

    Examples:
      "idf,"
      "freq,"
      "dl,"
      "avgdl,"
      "n,"
      "N,"
    """
    if node.get("description", "").startswith(description_prefix):
        return node.get("value")

    for child in node.get("details", []):
        value = find_metric(child, description_prefix)
        if value is not None:
            return value

    return None


def find_term_context(node: dict):
    """
    Find Term(field=..., "term") inside context.
    """
    for ctx in node.get("context", []) or []:
        match = TERM_RE.search(ctx)
        if match:
            return (match.group(1)), match.group(2)

    for child in node.get("details", []):
        result = find_term_context(child)
        if result is not None:
            return result

    return None


def get_boost(node: dict) -> float:
    match = BOOST_RE.search(node.get("description", ""))
    if match:
        return float(match.group(1))
    return 1.0


def get_base_score(node: dict) -> Any:
    """
    If this is a Boost node, the first child is usually the unboosted score.
    Otherwise the node value itself is the base score.
    """
    if node.get("description", "").startswith("Boost") and node.get("details"):
        return node["details"][0]["value"]
    return node.get("value")


def summarize_explanation(exp: dict) -> list[dict]:
    total = exp["value"]
    rows = []

    for clause_index, clause in enumerate(exp.get("details", []), start=1):
        # print("**** ", clause_index, clause)
        term_info = find_term_context(clause)

        if term_info is None:
            field = None
            term = None
        else:
            field, term = term_info

        score = clause.get("value")
        boost = get_boost(clause)
        base_score = get_base_score(clause)

        rows.append(
            {
                "clause": clause_index,
                "term": term,
                "field": field,
                "score": score,
                "percent": 100.0 * score / total if total else 0.0,
                "boost": boost,
                "base_score": base_score,
                "freq": find_metric(clause, "freq,"),
                "idf": find_metric(clause, "idf,"),
                "n": find_metric(clause, "n,"),
                "N": find_metric(clause, "N,"),
                "dl": find_metric(clause, "dl,"),
                "avgdl": find_metric(clause, "avgdl,"),
            }
        )

    return rows

def print_header():
    print(
        f"{'term':<12} "
        f"{'field':>5} "
        f"{'score':>9} "
        f"{'percent':>7} "
        f"{'boost':>6} "
        f"{'base':>9} "
        f"{'freq':>6} "
        f"{'idf':>8} "
        f"{'dl':>7} "
        f"{'avgdl':>9} "
        f"{'n':>7}"
    )




def print_grouped_summary(rows: list[dict]) -> None:
    by_term = defaultdict(float)
    by_field = defaultdict(float)

    for r in rows:
        by_term[r["term"]] += r["score"]
        by_field[r["field"]] += r["score"]

    print("\nScore by term:")
    for term, score in sorted(by_term.items(), key=lambda x: x[1], reverse=True):
        print(f"  {term:<12} {score:.3f}")

    print("\nScore by field:")
    for field, score in sorted(by_field.items(), key=lambda x: x[1], reverse=True):
        print(f"  field {field:<4} {score:.3f}")

def main() -> None:
    print_header()
    text = sys.stdin.read().strip()
    hits = json.loads(text)
    exp_indicator = "Explanation("
    for hit in hits:
        expl = hit.get('explanation')
        if isinstance(expl,str):
            start = expl.find(exp_indicator) + len(exp_indicator)
            for r in summarize_explanation(json.loads(expl[start:-1])):
                print(
                            f"{str(r['term']):<12} "
                            f"{str(r['field']):>5} "
                            f"{r['score']:>9.3f} "
                            f"{r['percent']:>6.1f}% "
                            f"{r['boost']:>6.1f} "
                            f"{r['base_score']:>9.3f} "
                            f"{str(r['freq']):>6} "
                            f"{r['idf']:>8.3f} "
                            f"{str(r['dl']):>7} "
                            f"{r['avgdl']:>9.3f} "
                            f"{str(r['n']):>7}"
                        )
            print('-----------------')
if __name__ == "__main__":
    main()
