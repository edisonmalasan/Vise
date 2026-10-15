# Graph Report - vise  (2026-10-15)

## Corpus Check
- Corpus is ~3,497 words - fits in a single context window. You may not need a graph.

## Summary
- 22 nodes · 17 edges · 10 communities (2 shown, 8 thin omitted)
- Extraction: 94% EXTRACTED · 6% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Response and Direct Manipulation
- Spring Physics and Momentum
- Frame Smoothness and Libraries
- Design Process
- Design Principles
- Initial Response
- Materials and Depth
- Multimodal Feedback
- Rubber Banded Boundaries
- Spatial Consistency

## God Nodes (most connected - your core abstractions)
1. `Apple Design Skill` - 7 edges
2. `Springs (behavior over animation)` - 6 edges
3. `Interruptibility` - 3 edges
4. `Velocity Handoff` - 3 edges
5. `Four Human Needs (safety/predictability, understanding, achievement, joy)` - 2 edges
6. `Response (kill latency)` - 2 edges
7. `Direct Manipulation (1:1 tracking)` - 2 edges
8. `Momentum Projection` - 2 edges
9. `Damping Ratio` - 1 edges
10. `Response Parameter (seconds, not duration)` - 1 edges

## Surprising Connections (you probably didn't know these)
- `Apple Design Skill` --covers--> `Interruptibility`  [EXTRACTED]
  .agents/skills/apple-design/SKILL.md → .agents/skills/apple-design/SKILL.md  _Bridges community 0 → community 1_

## Communities (10 total, 8 thin omitted)

### Community 0 - "Response and Direct Manipulation"
Cohesion: 0.33
Nodes (7): Apple Design Skill, Direct Manipulation (1:1 tracking), Four Human Needs (safety/predictability, understanding, achievement, joy), Pointer Events / setPointerCapture, Reduced Motion & Accessibility, Response (kill latency), Typography (optical sizing, tracking, leading)

### Community 1 - "Spring Physics and Momentum"
Cohesion: 0.47
Nodes (6): Damping Ratio, Interruptibility, Momentum Projection, Response Parameter (seconds, not duration), Springs (behavior over animation), Velocity Handoff

## Knowledge Gaps
- **14 isolated node(s):** `Initial Response`, `Damping Ratio`, `Response Parameter (seconds, not duration)`, `Spatial Consistency (symmetric paths, anchored origins)`, `Rubber-banding (soft boundaries)` (+9 more)
  These have ≤1 connection - possible missing edges. (Counts symbols only; 14 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Apple Design Skill` connect `Response and Direct Manipulation` to `Spring Physics and Momentum`?**
  _High betweenness centrality (0.233) - this node is a cross-community bridge._
- **What connects `Initial Response`, `Damping Ratio`, `Response Parameter (seconds, not duration)` to the rest of the system?**
  _14 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Why does `Springs (behavior over animation)` connect `Spring Physics and Momentum` to `Response and Direct Manipulation`?**
  _High betweenness centrality (0.152) - this node is a cross-community bridge._