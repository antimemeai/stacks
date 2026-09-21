# stacks

The brain for the bench: making the library, data, and the inventory/catalog of
equipment and reagents accessible and useful to the experimental and industrial
processes this server runs.

## Layout

- `papers/` — acquired literature. Ignored by Git.
- `quarantine/` — acquired reference implementations. Ignored by Git. Material
  in here is historical/reference only; copying a project here does not adopt
  its architecture, dependencies, or agent procedures.

A future checkout must be able to identify any external material an experiment
needs; acquisition manifests live in versioned files outside those directories
(see `docs/` once they exist).

## Relationship to neurotic_library

`~/neurotic_library` is sunsetted: not deleted, not modified, and full of
material this project becomes the new home for. Migration happens deliberately,
piece by piece, as a need and a check justify each move.

## Direction (from the user, 2026-09-21)

- **Rust is core, everything.** Departures need a concrete, argued reason.
- The web interface is exposed over **Tailscale**; the Go tailscale toolchain
  (tsnet-style) is the accepted non-Rust exception for that edge.
- **APIs and LLM ergonomics/comfort are first-class design requirements.** This
  system will be operated by and with models, not just humans.
- **Interfaces everywhere.** stacks is one part of an armada of lab software;
  it must compose, not monopolize. Boundaries are the product.

## Workflow

research → design → plan → execute, with a conversation at every phase
boundary. We are in **research** (acquisition) now.
