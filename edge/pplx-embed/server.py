"""pplx-embed sidecar: perplexity-ai/pplx-embed-v2-context-9b-preview over HTTP.

Deliberate Python exception to the stacks Rust-core rule: the embedding model
requires torch/transformers. Runs on the GPU laptop, exposed over Tailscale.
"""

import logging
import os
import time

import numpy as np
import torch
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from transformers import AutoModel

MODEL_ID = "perplexity-ai/pplx-embed-v2-context-9b-preview"
MAX_CHUNKS = 128
MAX_CHARS = 1_000_000

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
log = logging.getLogger("pplx-embed")

app = FastAPI(title="pplx-embed")

model = None
revision = "unknown"
dims = 2048
device = "cuda"


class DocumentsRequest(BaseModel):
    documents: list[list[str]]


class QueriesRequest(BaseModel):
    queries: list[str]


def _guard(total_chunks: int, texts) -> None:
    total_chars = sum(len(t) for t in texts)
    if total_chunks > MAX_CHUNKS or total_chars > MAX_CHARS:
        raise HTTPException(
            status_code=413,
            detail=f"request too large: {total_chunks} chunks / {total_chars} chars "
            f"(limits {MAX_CHUNKS} / {MAX_CHARS})",
        )


def _to_int8(arr: np.ndarray) -> list[list[int]]:
    # The model card says native output is UNNORMALIZED int8-quantized values
    # in a float container. This code assumes values are already integer-valued
    # (rounding is then a no-op) and clamps to the int8 range defensively.
    a = np.rint(np.asarray(arr, dtype=np.float64))
    a = np.clip(a, -128, 127).astype(np.int8)
    return a.astype(int).tolist()


@app.on_event("startup")
def load_model() -> None:
    global model, revision, device
    device = "cuda" if torch.cuda.is_available() else "cpu"
    if device != "cuda":
        log.warning("CUDA not available; falling back to CPU")
    try:
        from huggingface_hub import model_info

        revision = model_info(MODEL_ID).sha or "unknown"
    except Exception as e:
        log.warning("could not resolve HF revision: %s", e)
    t0 = time.monotonic()
    model = AutoModel.from_pretrained(MODEL_ID, trust_remote_code=True).to(device)
    log.info(
        "model loaded: %s rev=%s device=%s dtype=%s in %.1fs",
        MODEL_ID,
        revision,
        device,
        next(model.parameters()).dtype,
        time.monotonic() - t0,
    )


@app.get("/healthz")
def healthz():
    return {
        "status": "ok",
        "model": MODEL_ID,
        "revision": revision,
        "dims": dims,
        "device": device,
    }


@app.post("/encode_documents")
def encode_documents(req: DocumentsRequest):
    flat = [c for doc in req.documents for c in doc]
    _guard(len(flat), flat)
    t0 = time.monotonic()
    arrays = model.encode(req.documents, normalize_embeddings=False)
    result = [_to_int8(a) for a in arrays]
    log.info(
        "encode_documents docs=%d chunks=%d %.3fs",
        len(req.documents),
        len(flat),
        time.monotonic() - t0,
    )
    return {"embeddings": result}


@app.post("/encode_queries")
def encode_queries(req: QueriesRequest):
    _guard(len(req.queries), req.queries)
    t0 = time.monotonic()
    wrapped = [[q] for q in req.queries]
    arrays = model.encode_queries(wrapped, normalize_embeddings=False)
    result = [_to_int8(a[0]) for a in arrays]
    log.info("encode_queries n=%d %.3fs", len(req.queries), time.monotonic() - t0)
    return {"embeddings": result}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        app,
        host=os.environ.get("PPLX_BIND", "127.0.0.1"),
        port=int(os.environ.get("PPLX_PORT", "8569")),
    )
