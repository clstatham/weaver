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
    uid_to_node: SortedMap<K, NodeIndex>,
    node_to_uid: BTreeMap<NodeIndex, K>,
}

impl<K: Ord + Eq + Clone, R> Default for Graph<K, R> {
    fn default() -> Self {
        Self {
            graph: StableGraph::new(),
            uid_to_node: SortedMap::default(),
            node_to_uid: BTreeMap::default(),
        }
    }
}

impl<K: Ord + Eq + Clone, R: Clone> Graph<K, R> {
    pub fn add_node(&mut self, uid: K) -> bool {
        if self.uid_to_node.contains(&uid) {
            return false;
        }
        let node = self.graph.add_node(uid.clone());
        self.uid_to_node.insert(uid.clone(), node);
        self.node_to_uid.insert(node, uid.clone());
        true
    }

    pub fn remove_node(&mut self, uid: &K) -> bool {
        if let Some(node) = self.uid_to_node.remove(uid) {
            self.node_to_uid.remove(&node);
            self.graph.remove_node(node).is_some()
        } else {
            false
        }
    }

    #[must_use = "Adding a child may fail if the parent or child does not exist"]
    pub fn add_child(&mut self, parent: &K, child: &K, payload: R) -> Option<Edge<K, R>> {
        let from_node = *self.uid_to_node.get(parent)?;
        let to_node = *self.uid_to_node.get(child)?;
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
        let mut bfs = Bfs::new(&self.graph, *self.uid_to_node.get(&starts[0])?);
        for start in starts.iter().skip(1) {
            bfs.stack.push_back(*self.uid_to_node.get(start)?);
        }
        Some(std::iter::from_fn(move || {
            bfs.next(&self.graph)
                .map(|node| self.node_to_uid.get(&node).unwrap())
        }))
    }

    pub fn dfs(&self, start: K) -> Option<impl Iterator<Item = &K> + '_> {
        let mut dfs = Dfs::new(&self.graph, *self.uid_to_node.get(&start)?);
        Some(std::iter::from_fn(move || {
            dfs.next(&self.graph)
                .map(|node| self.node_to_uid.get(&node).unwrap())
        }))
    }

    pub fn get_parents(&self, uid: &K) -> Option<impl Iterator<Item = &K> + '_> {
        let node = *self.uid_to_node.get(uid)?;
        let mut neighbors = self.graph.neighbors_directed(node, Direction::Incoming);
        Some(std::iter::from_fn(move || {
            neighbors
                .next()
                .map(|node| self.node_to_uid.get(&node).unwrap())
        }))
    }

    pub fn get_children(&self, uid: &K) -> Option<impl Iterator<Item = &K> + '_> {
        let node = *self.uid_to_node.get(uid)?;
        let mut neighbors = self.graph.neighbors_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || {
            neighbors
                .next()
                .map(|node| self.node_to_uid.get(&node).unwrap())
        }))
    }

    pub fn get_child_edges(&self, uid: &K) -> Option<impl Iterator<Item = &Edge<K, R>> + '_> {
        let node = *self.uid_to_node.get(uid)?;
        let mut edges = self.graph.edges_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || edges.next().map(|e| e.weight())))
    }

    pub fn get_edges(&self, parent: &K, child: &K) -> Option<impl Iterator<Item = &Edge<K, R>>> {
        let from_node = *self.uid_to_node.get(parent)?;
        let to_node = *self.uid_to_node.get(child)?;
        let edges = self.graph.edges_connecting(from_node, to_node);
        let edges = edges.map(|edge| edge.weight());
        Some(edges)
    }

    pub fn contains(&self, uid: &K) -> bool {
        self.uid_to_node.contains(uid)
    }

    pub fn len(&self) -> usize {
        self.uid_to_node.len()
    }

    pub fn is_empty(&self) -> bool {
        self.uid_to_node.is_empty()
    }

    pub fn clear(&mut self) {
        self.graph.clear();
        self.uid_to_node.clear();
        self.node_to_uid.clear();
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = &K> + '_ {
        self.uid_to_node.iter().map(|(uid, _)| uid)
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = &Edge<K, R>> + '_ {
        self.graph.edge_references().map(|edge| edge.weight())
    }

    pub fn iter_child_edges(&self, uid: K) -> Option<impl Iterator<Item = &Edge<K, R>> + '_> {
        let node = *self.uid_to_node.get(&uid)?;
        let mut edges = self.graph.edges_directed(node, Direction::Outgoing);
        Some(std::iter::from_fn(move || {
            edges.next().map(|edge| edge.weight())
        }))
    }

    pub fn orphans(&self) -> Vec<K> {
        let mut orphans = Vec::new();
        for (uid, node) in self.uid_to_node.iter() {
            if self
                .graph
                .neighbors_directed(*node, Direction::Incoming)
                .next()
                .is_none()
            {
                orphans.push(uid.clone());
            }
        }
        orphans
    }

    pub fn remove_node_recursive(&mut self, uid: &K) -> Option<bool> {
        let node = *self.uid_to_node.get(uid)?;
        let mut neighbors = self
            .graph
            .neighbors_directed(node, Direction::Outgoing)
            .collect::<Vec<_>>();
        for child in neighbors.drain(..) {
            let child_uid = self.node_to_uid.get(&child)?.clone();
            self.remove_node_recursive(&child_uid);
        }
        Some(self.remove_node(uid))
    }

    pub fn ancestors(&self, uid: &K) -> Option<impl Iterator<Item = K> + '_> {
        let mut ancestors = Vec::new();
        let mut current = uid;
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
