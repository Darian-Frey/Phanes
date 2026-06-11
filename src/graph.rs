//! The relationship graph: nodes are ideas, edges are the three relationship
//! kinds — explicit links, shared tags, and semantic neighbours. Deterministic:
//! semantic edges use the stored vectors + cosine, so no model runs here
//! (INV-1), and nothing is persisted (the graph is rebuilt from the index on
//! demand — INV-3). Powers the UI graph view and the `gaps` analysis.

use std::collections::HashMap;

use anyhow::Result;

use crate::model::Status;
use crate::query::{self, cosine};
use crate::store::Store;

/// What kind of relationship an edge represents. Ordered by authority for merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// An explicit `[[wikilink]]` / `.md` link (asserted).
    Link,
    /// Shared asserted/proposed tags.
    Tag,
    /// High cosine similarity between embeddings (proposed adjacency).
    Semantic,
}

fn priority(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Link => 3,
        EdgeKind::Tag => 2,
        EdgeKind::Semantic => 1,
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub title: String,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub a: usize, // node index
    pub b: usize, // node index (a < b)
    pub weight: f32, // 0..1
    pub kind: EdgeKind,
}

#[derive(Debug, Clone)]
pub struct RelGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Knobs for edge construction.
pub struct GraphOptions {
    /// Minimum cosine similarity (0..1) for a semantic edge.
    pub semantic_threshold: f32,
    /// Keep at most this many (strongest) semantic edges per node — avoids a
    /// hairball when everything is mildly similar to everything.
    pub semantic_per_node: usize,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self { semantic_threshold: 0.6, semantic_per_node: 4 }
    }
}

fn add_edge(
    merged: &mut HashMap<(usize, usize), (f32, EdgeKind)>,
    a: usize,
    b: usize,
    weight: f32,
    kind: EdgeKind,
) {
    if a == b {
        return;
    }
    let key = if a < b { (a, b) } else { (b, a) };
    merged
        .entry(key)
        .and_modify(|e| {
            if priority(kind) > priority(e.1) {
                e.1 = kind;
            }
            if weight > e.0 {
                e.0 = weight;
            }
        })
        .or_insert((weight, kind));
}

/// Build the relationship graph from the index. One undirected edge per pair,
/// tagged with the strongest relationship kind present.
pub fn build(store: &Store, opts: &GraphOptions) -> Result<RelGraph> {
    let nodes: Vec<Node> = query::list(store)?
        .into_iter()
        .map(|i| Node { id: i.id, title: i.title, status: i.status })
        .collect();
    let idx: HashMap<String, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();

    let mut merged: HashMap<(usize, usize), (f32, EdgeKind)> = HashMap::new();

    // Explicit links.
    {
        let mut stmt = store.conn.prepare("SELECT src_id, dst_id FROM links")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (s, d) = row?;
            if let (Some(&a), Some(&b)) = (idx.get(&s), idx.get(&d)) {
                add_edge(&mut merged, a, b, 1.0, EdgeKind::Link);
            }
        }
    }

    // Shared tags (count -> weight).
    {
        let mut stmt = store.conn.prepare(
            "SELECT t1.idea_id, t2.idea_id, COUNT(*) \
               FROM tags t1 JOIN tags t2 \
                 ON t1.tag = t2.tag AND t1.idea_id < t2.idea_id \
              GROUP BY t1.idea_id, t2.idea_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (s, d, count) = row?;
            if let (Some(&a), Some(&b)) = (idx.get(&s), idx.get(&d)) {
                let weight = (count as f32 / 3.0).min(1.0);
                add_edge(&mut merged, a, b, weight, EdgeKind::Tag);
            }
        }
    }

    // Semantic edges: top-k strongest neighbours per node above the threshold.
    {
        let vecs: HashMap<usize, Vec<f32>> = store
            .all_embeddings()?
            .into_iter()
            .filter_map(|(id, v)| idx.get(&id).map(|&i| (i, v)))
            .collect();
        let ids: Vec<usize> = vecs.keys().copied().collect();
        for &i in &ids {
            let vi = &vecs[&i];
            let mut sims: Vec<(usize, f32)> = ids
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| (j, cosine(vi, &vecs[&j])))
                .filter(|&(_, c)| c >= opts.semantic_threshold)
                .collect();
            sims.sort_by(|x, y| y.1.total_cmp(&x.1));
            sims.truncate(opts.semantic_per_node);
            for (j, c) in sims {
                add_edge(&mut merged, i, j, c, EdgeKind::Semantic);
            }
        }
    }

    let edges = merged
        .into_iter()
        .map(|((a, b), (weight, kind))| Edge { a, b, weight, kind })
        .collect();
    Ok(RelGraph { nodes, edges })
}

impl RelGraph {
    /// Edge count per node.
    pub fn degree(&self) -> Vec<usize> {
        let mut deg = vec![0usize; self.nodes.len()];
        for e in &self.edges {
            deg[e.a] += 1;
            deg[e.b] += 1;
        }
        deg
    }

    /// Connected-component label per node (union-find). Same label = same cluster.
    pub fn components(&self) -> Vec<usize> {
        let n = self.nodes.len();
        let mut parent: Vec<usize> = (0..n).collect();
        for e in &self.edges {
            let ra = find(&mut parent, e.a);
            let rb = find(&mut parent, e.b);
            if ra != rb {
                parent[ra] = rb;
            }
        }
        (0..n).map(|i| find(&mut parent, i)).collect()
    }

    /// Nodes with no edges at all — ideas connected to nothing.
    pub fn orphans(&self) -> Vec<usize> {
        let deg = self.degree();
        (0..self.nodes.len()).filter(|&i| deg[i] == 0).collect()
    }

    /// "Should connect but don't": the strongest semantic pairs that aren't also
    /// captured by an explicit link or shared tag (those merge to a higher kind).
    /// Sorted by similarity, strongest first.
    pub fn bridges(&self, limit: usize) -> Vec<&Edge> {
        let mut b: Vec<&Edge> =
            self.edges.iter().filter(|e| e.kind == EdgeKind::Semantic).collect();
        b.sort_by(|x, y| y.weight.total_cmp(&x.weight));
        b.truncate(limit);
        b
    }
}

/// Union-find root with path compression.
fn find(parent: &mut [usize], x: usize) -> usize {
    let mut root = x;
    while parent[root] != root {
        root = parent[root];
    }
    let mut cur = x;
    while parent[cur] != root {
        let next = parent[cur];
        parent[cur] = root;
        cur = next;
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Idea, Sourced};
    use crate::store::{Store, SCHEMA};
    use chrono::Utc;
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn mem_store() -> Store {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Store { conn }
    }

    fn idea(id: &str, links: &[&str]) -> Idea {
        Idea {
            id: id.into(),
            path: PathBuf::from(format!("/i/{id}.md")),
            title: id.into(),
            status: Sourced::asserted(Status::Active),
            summary: None,
            tags: Vec::new(),
            topics: Vec::new(),
            last_reviewed: None,
            mtime: Utc::now(),
            content_hash: format!("h-{id}"),
            body: id.into(),
            links: links.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn node_idx(g: &RelGraph, id: &str) -> usize {
        g.nodes.iter().position(|n| n.id == id).unwrap()
    }

    #[test]
    fn builds_link_and_semantic_edges_and_finds_gaps() {
        let mut store = mem_store();
        // a -> b explicit link; a,b near each other; c near d; e isolated.
        store.upsert(&idea("a", &["b"])).unwrap();
        store.upsert(&idea("b", &[])).unwrap();
        store.upsert(&idea("c", &[])).unwrap();
        store.upsert(&idea("d", &[])).unwrap();
        store.upsert(&idea("e", &[])).unwrap();
        store.set_embedding("a", &[1.0, 0.0]).unwrap();
        store.set_embedding("b", &[0.98, 0.20]).unwrap(); // near a
        store.set_embedding("c", &[0.0, 1.0]).unwrap();
        store.set_embedding("d", &[0.10, 0.99]).unwrap(); // near c
        store.set_embedding("e", &[-1.0, 0.0]).unwrap(); // opposite a/b, orthogonal to c/d: near nothing > 0.6

        let g = build(&store, &GraphOptions::default()).unwrap();

        // a-b is both linked and semantic -> merges to the Link kind.
        let ab = g
            .edges
            .iter()
            .find(|e| {
                (e.a == node_idx(&g, "a") && e.b == node_idx(&g, "b"))
                    || (e.a == node_idx(&g, "b") && e.b == node_idx(&g, "a"))
            })
            .unwrap();
        assert_eq!(ab.kind, EdgeKind::Link);

        // c-d is a semantic-only "bridge".
        let bridges = g.bridges(10);
        assert!(bridges.iter().any(|e| {
            let (x, y) = (g.nodes[e.a].id.as_str(), g.nodes[e.b].id.as_str());
            (x == "c" && y == "d") || (x == "d" && y == "c")
        }));
        // the linked pair a-b is NOT a bridge (it's already linked).
        assert!(!bridges.iter().any(|e| {
            let (x, y) = (g.nodes[e.a].id.as_str(), g.nodes[e.b].id.as_str());
            (x == "a" && y == "b") || (x == "b" && y == "a")
        }));

        // e connects to nothing.
        let orphans: Vec<&str> = g.orphans().iter().map(|&i| g.nodes[i].id.as_str()).collect();
        assert_eq!(orphans, vec!["e"]);

        // {a,b} and {c,d} are two clusters; e is its own.
        let comp = g.components();
        assert_eq!(comp[node_idx(&g, "a")], comp[node_idx(&g, "b")]);
        assert_eq!(comp[node_idx(&g, "c")], comp[node_idx(&g, "d")]);
        assert_ne!(comp[node_idx(&g, "a")], comp[node_idx(&g, "c")]);
        assert_ne!(comp[node_idx(&g, "e")], comp[node_idx(&g, "a")]);
    }
}
