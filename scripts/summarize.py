#!/usr/bin/env python3

import json
import re
from collections import defaultdict
import sys
from typing import Any


TERM_RE = re.compile(r'Term=Term\(field=(\d+), type=\w+, "([^"]+)"\)')
BOOST_RE = re.compile(r"Boost x([0-9.]+) of")


def extract_explanations(text: str) -> list[dict]:
    """
    Extract JSON objects from Tantivy-style:

        Explanation({
          "value": ...
        })

    Handles nested JSON braces and strings.
    """
    explanations = []
    marker = "Explanation("
    i = 0

    while True:
        start = text.find(marker, i)
        if start == -1:
            break

        j = start + len(marker)

        while j < len(text) and text[j].isspace():
            j += 1

        if j >= len(text) or text[j] != "{":
            i = j
            continue

        depth = 0
        in_string = False
        escaped = False

        for k in range(j, len(text)):
            ch = text[k]

            if in_string:
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == '"':
                    in_string = False
            else:
                if ch == '"':
                    in_string = True
                elif ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        raw_json = text[j : k + 1]
                        explanations.append(json.loads(raw_json))
                        i = k + 1
                        break
        else:
            raise ValueError("Unclosed Explanation({...}) block")

    return explanations


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
            return int(match.group(1)), match.group(2)

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


def summarize_explanation(exp: dict, explanation_index: int) -> list[dict]:
    total = exp["value"]
    rows = []

    for clause_index, clause in enumerate(exp.get("details", []), start=1):
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
                "explanation": explanation_index,
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


def print_table(rows: list[dict]) -> None:
    headers = [
        "expl",
        "term",
        "field",
        "score",
        "%",
        "boost",
        "base",
        "freq",
        "idf",
        "dl",
        "avgdl",
        "n",
    ]

    print(
        f"{headers[0]:>4}  {headers[1]:<12} {headers[2]:>5} "
        f"{headers[3]:>9} {headers[4]:>7} {headers[5]:>6} "
        f"{headers[6]:>9} {headers[7]:>6} {headers[8]:>8} "
        f"{headers[9]:>7} {headers[10]:>9} {headers[11]:>7}"
    )

    print("-" * 110)

    for r in rows:
        print(
            f"{r['explanation']:>4}  "
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

    text = sys.stdin.read().strip()
    explanations = extract_explanations(text)

    if not explanations:
        raise SystemExit("No Explanation({...}) blocks found")

    all_rows = []

    for i, exp in enumerate(explanations, start=1):
        print(f"\nExplanation #{i}")
        print(f"Total score: {exp['value']:.3f}")
        print(f"Description: {exp.get('description', '')}")

        rows = summarize_explanation(exp, i)
        print_table(rows)
        print_grouped_summary(rows)

        all_rows.extend(rows)


if __name__ == "__main__":
    main()
