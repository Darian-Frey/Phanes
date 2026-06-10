---
status: active
tags: [ui, visualization, spatial]
last_reviewed: 2026-05-28
---

# Spatial Canvas for Idea Relationships

A pan-and-zoom canvas that lays ideas out as nodes and their explicit links as
edges, so the relationship layer is something you *see* rather than query. Shared
tags pull related notes into loose clusters; dragging a node and dropping it near
another could propose a link.

This is the spatial-first view sketched in the roadmap's P4. It reads the same
computed neighbours the `related` command returns — nothing new is stored.

Relates to the [LLM idea graph](llm-idea-graph.md) idea, and to [[Spatial Canvas]]
itself as the anchor concept.
