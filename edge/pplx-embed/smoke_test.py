"""Smoke test against a running pplx-embed server.

Usage: PPLX_HOST=<tailscale-ip> python smoke_test.py
Requires: requests (pip install requests), numpy.
"""

import os
import sys

import numpy as np
import requests

HOST = os.environ.get("PPLX_HOST", "127.0.0.1")
PORT = int(os.environ.get("PPLX_PORT", "8569"))
BASE = f"http://{HOST}:{PORT}"
DIMS = 2048

DOCS = [
    [
        "Aluminum 6061-T6 has a tensile strength of about 310 MPa.",
        "It is commonly used for structural frames and fittings.",
        "The alloy is heat-treatable and weldable.",
    ],
    [
        "The French Revolution began in 1789.",
        "Paris was its political center.",
    ],
]
QUERY = "what is the tensile strength of aluminum 6061?"


def check_vector(v, label):
    assert isinstance(v, list) and len(v) == DIMS, f"{label}: expected {DIMS} dims, got {len(v)}"
    assert all(isinstance(x, int) for x in v), f"{label}: non-int value present"
    assert all(-128 <= x <= 127 for x in v), f"{label}: value out of int8 range"


def main():
    r = requests.get(f"{BASE}/healthz", timeout=30)
    r.raise_for_status()
    health = r.json()
    print("healthz:", health)
    assert health["status"] == "ok" and health["dims"] == DIMS

    r = requests.post(f"{BASE}/encode_documents", json={"documents": DOCS}, timeout=300)
    r.raise_for_status()
    doc_embs = r.json()["embeddings"]
    assert len(doc_embs) == 2
    assert len(doc_embs[0]) == 3 and len(doc_embs[1]) == 2
    for i, doc in enumerate(doc_embs):
        for j, v in enumerate(doc):
            check_vector(v, f"doc[{i}][{j}]")

    r = requests.post(f"{BASE}/encode_queries", json={"queries": [QUERY]}, timeout=300)
    r.raise_for_status()
    q_emb = r.json()["embeddings"][0]
    check_vector(q_emb, "query")

    q = np.asarray(q_emb, dtype=np.float64)
    q /= np.linalg.norm(q)
    scored = []
    for i, doc in enumerate(doc_embs):
        for j, v in enumerate(doc):
            c = np.asarray(v, dtype=np.float64)
            c /= np.linalg.norm(c)
            scored.append((float(q @ c), i, j))
    scored.sort(reverse=True)
    print("query-vs-chunk cosine ranking:")
    for score, i, j in scored:
        print(f"  {score:+.4f}  doc[{i}][{j}]  {DOCS[i][j][:60]}")

    top_i = scored[0][1]
    if top_i != 0:
        print("WARN: top-ranked chunk is not from the aluminum document", file=sys.stderr)
        sys.exit(1)
    print("OK")


if __name__ == "__main__":
    main()
