use std::collections::BTreeMap;

use petgraph::{prelude::*, visit::IntoEdgeReferences};

use crate::storage::SortedMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Edge<K: Ord + Eq + Clone, R> {
    pub parent: K,
    pub child: K,
    pub payload: R,
    id: EdgeIndex,
}

#[derive(Debug, Clone)]
pub struct Graph<K: Ord + Eq + Clone, R> {
    graph: StableGraph<K, Edge<K, R>, Directed>,
    key_to_node: SortedMap<K, NodeIndex>,
    node_to_key: BTreeMap<NodeIndex, K>,
}

impl<K: Ord + Eq + Clone, R> Default for Graph<K, R> {
    fn default() -> Self {
        Self {
            graph: StableGraph::new(),
            key_to_node: SortedMap::default(),
            node_to_key: BTreeMap::default(),
        }
    }
}

impl<K: Ord + Eq + Clone, R: Clone> Graph<K, R> {
    pub fn add_node(&mut self, key: K) -> bool {
        if self.key_to_node.contains(&key) {
            return false;
        }
        let node = self.graph.add_node(key.clone());
        self.key_to_node.insert(key.clone(), node);
        self.node_to_key.insert(node, key.clone());
        true
    }

    pub fn remove_node(&mut self, key: &K) -> bool {
        if let Some(node) = self.key_to_node.remove(key) {
            self.node_to_key.remove(&node);
            self.graph.remove_node(node).is_some()
        } else {
            false
        }
    }

    #[must_use = "Adding a child may fail if the parent or child does not exist"]
    pub fn add_child(&mut self, parent: &K, child: &K, payload: R) -> Option<Edge<K, R>> {
        let from_node = *self.key_to_node.get(parent)?;
        let to_node = *self.key_to_node.get(child)?;
        let edge = Edge {
            parent: parent.clone(),
            child: child.clone(),
            payload,
            id: Default::default(),
        };
        let id = self.graph.add_edge(from_node, to_node, edge.clone());
        self.graph.edge_weight_mut(id)?.id = id;
        Some(edge)
    }

    pub fn remove_edge(&mut self, edge: Edge<K, R>) -> bool {
        self.graph.remove_edge(edge.id).is_some()
    }

    pub fn bfs(
        &self,
        starts: impl IntoIterator<Item = K>,
    ) -> Option<impl Iterator<Item = &K> + '_> {
        let starts = starts.into_iter().collect::<Vec<_>>();
        let mut bfs = Bfs::new(&self.graph, *self.key_to_node.get(&starts[0])?);
        for start in starts.iter().skip(1) {
            bfs.stack.push_back(*self.key_to_node.get(start)?);
        }
        Some(std::iter::from_fn(move || {
            bfs.next(&self.graph)
                .map(|node| self.node_to_key.get(&node).unwrap())
        }))
    }

    pub fn dfs(&self, start: K) -> Option<impl Iterator<Item = &K> + '_> {
        let mut dfs = Dfs::new(&self.graph, *self.key_to_node.get(&start)?);
        Some(std::iter::from_fn(move || {
            dfs.next(&self.graph)
                .map(|node| self.node_to_key.get(&node).unwrap())
        }))
    }

    pub fn get_parents(&self, key: &K) -> Option<impl Iterator<Item = &K> + '_> {
        let node = *self.key_to_node.get(key)?;
        let mut neighbors = self.graph.neighbors_directed(node, Direction::Incoming);
        Some(std::iter::from_fn(move || {
            neighbors
                .next()
                .map(|node| self.node_to_key.get(&node).unwrap())
        }))
    }

    pub fn get_children(&self, key: &K) -> Option<impl Iterator<Item = &K> + '_> {
        let node = *self.key_to_node.get(key)?;
        let mut neighbors = self.graph.neighbors_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || {
            neighbors
                .next()
                .map(|node| self.node_to_key.get(&node).unwrap())
        }))
    }

    pub fn get_child_edges(&self, key: &K) -> Option<impl Iterator<Item = &Edge<K, R>> + '_> {
        let node = *self.key_to_node.get(key)?;
        let mut edges = self.graph.edges_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || edges.next().map(|e| e.weight())))
    }

    pub fn get_edges(&self, parent: &K, child: &K) -> Option<impl Iterator<Item = &Edge<K, R>>> {
        let from_node = *self.key_to_node.get(parent)?;
        let to_node = *self.key_to_node.get(child)?;
        let edges = self.graph.edges_connecting(from_node, to_node);
        let edges = edges.map(|edge| edge.weight());
        Some(edges)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.key_to_node.contains(key)
    }

    pub fn len(&self) -> usize {
        self.key_to_node.len()
    }

    pub fn is_empty(&self) -> bool {
        self.key_to_node.is_empty()
    }

    pub fn clear(&mut self) {
        self.graph.clear();
        self.key_to_node.clear();
        self.node_to_key.clear();
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = &K> + '_ {
        self.key_to_node.iter().map(|(key, _)| key)
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = &Edge<K, R>> + '_ {
        self.graph.edge_references().map(|edge| edge.weight())
    }

    pub fn iter_child_edges(&self, key: K) -> Option<impl Iterator<Item = &Edge<K, R>> + '_> {
        let node = *self.key_to_node.get(&key)?;
        let mut edges = self.graph.edges_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || {
            edges.next().map(|edge| edge.weight())
        }))
    }

    pub fn orphans(&self) -> Vec<K> {
        let mut orphans = Vec::new();
        for (key, node) in self.key_to_node.iter() {
            if self
                .graph
                .neighbors_directed(*node, Direction::Incoming)
                .next()
                .is_none()
            {
                orphans.push(key.clone());
            }
        }
        orphans
    }

    pub fn remove_node_recursive(&mut self, key: &K) -> Option<bool> {
        let node = *self.key_to_node.get(key)?;
        let mut neighbors = self
            .graph
            .neighbors_directed(node, Direction::Outgoing)
            .collect::<Vec<_>>();
        for child in neighbors.drain(..) {
            let child_key = self.node_to_key.get(&child)?.clone();
            self.remove_node_recursive(&child_key);
        }
        Some(self.remove_node(key))
    }

    pub fn ancestors(&self, key: &K) -> Option<impl Iterator<Item = K> + '_> {
        let mut ancestors = Vec::new();
        let mut current = key;
        let mut stack = Vec::new();
        while let Some(mut parents) = self.get_parents(current) {
            while let Some(parent) = parents.next() {
                stack.push(parent.clone());
                current = parent;
                parents = self.get_parents(current)?;
            }
            while let Some(parent) = stack.pop() {
                ancestors.push(parent);
            }
        }
        Some(ancestors.into_iter())
    }
}
