# pplx-embed sidecar

Laptop inference sidecar for `perplexity-ai/pplx-embed-v2-context-9b-preview`.
Deliberate Python exception to the repo's Rust-core rule (torch/transformers
required; the tailscale toolchain is the other accepted exception).

Runs on the GPU laptop, reached from the server over Tailscale.

## Setup

    python3 -m venv .venv
    . .venv/bin/activate
    pip install -r requirements.txt

Requires transformers>=5.4.0 and a CUDA GPU. Memory/VRAM footprint of this 9B
model is unverified here — watch RAM/VRAM on the first load and record the
numbers in an operator note.

## Run

Bind to the laptop's Tailscale IPv4 so the server can reach it:

    TS_IP=$(tailscale ip -4)
    PPLX_BIND=$TS_IP python server.py

Defaults: `PPLX_BIND=127.0.0.1`, `PPLX_PORT=8569`.

First run downloads the model from HuggingFace; expect a long startup.

## Endpoints

- `GET /healthz` — status, model id, resolved HF revision, dims (2048), device.
- `POST /encode_documents` — `{"documents": [[chunk, ...], ...]}`; chunks of a
  document are encoded together (contextual). Returns int8 vectors mirroring
  the input structure.
- `POST /encode_queries` — `{"queries": [str, ...]}`; returns one int8 vector
  per query.

Guards: max 128 chunks and 1,000,000 chars per request (HTTP 413 beyond that).

Embeddings are the model's native unnormalized int8-quantized values; they are
not unit-normalized, so normalize client-side before cosine similarity.

## Smoke

    curl -s http://$TS_IP:8569/healthz
    curl -s -X POST http://$TS_IP:8569/encode_documents \
      -H 'content-type: application/json' \
      -d '{"documents": [["first chunk", "second chunk"], ["other doc chunk"]]}'
    curl -s -X POST http://$TS_IP:8569/encode_queries \
      -H 'content-type: application/json' \
      -d '{"queries": ["what is the tensile strength of aluminum 6061?"]}'

Or, with the venv active and the server running:

    PPLX_HOST=$TS_IP python smoke_test.py
