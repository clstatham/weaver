use std::{
    any::TypeId,
    collections::HashMap,
    fmt::Debug,
    ops::{Index, IndexMut},
    sync::Arc,
};

use petgraph::prelude::*;
use weaver_ecs::world::World;
use weaver_util::lock::Lock;

use crate::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderEdge {
    pub from_node: NodeIndex,
    pub from_slot: usize,
    pub to_node: NodeIndex,
    pub to_slot: usize,
}

pub trait Render: 'static + Send + Sync {
    #[allow(unused_variables)]
    fn prepare(&self, world: Arc<World>, renderer: &Renderer) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &self,
        world: Arc<World>,
        renderer: &Renderer,
        input_slots: &[Slot],
    ) -> anyhow::Result<Vec<Slot>>;
}

#[derive(Clone)]
pub enum Slot {
    Buffer(Arc<wgpu::Buffer>),
    Texture(Arc<wgpu::TextureView>),
    BindGroup(Arc<wgpu::BindGroup>),
}

pub struct RenderNode {
    name: String,
    render: Arc<Lock<Box<dyn Render>>>,
    render_type_id: TypeId,
}

impl Debug for RenderNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderNode")
            .field("name", &self.name)
            .finish()
    }
}

impl RenderNode {
    pub fn new<T: Render>(name: &str, render: T) -> Self {
        Self {
            name: name.to_string(),
            render: Arc::new(Lock::new(Box::new(render))),
            render_type_id: TypeId::of::<T>(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render_type_id(&self) -> TypeId {
        self.render_type_id
    }

    pub fn prepare(&self, world: Arc<World>, renderer: &Renderer) -> anyhow::Result<()> {
        self.render.write().prepare(world, renderer)
    }

    pub fn render(
        &self,
        world: Arc<World>,
        renderer: &Renderer,
        input_slots: &[Slot],
    ) -> anyhow::Result<Vec<Slot>> {
        self.render.write().render(world, renderer, input_slots)
    }
}

pub struct StartNode;

impl Render for StartNode {
    fn render(
        &self,
        _world: Arc<World>,
        renderer: &Renderer,
        _input_slots: &[Slot],
    ) -> anyhow::Result<Vec<Slot>> {
        // todo: don't assume we want to render to the main window (allow input slots to specify target)
        let current_frame = renderer.current_frame().unwrap();

        Ok(vec![
            Slot::Texture(current_frame.color_view),
            Slot::Texture(current_frame.depth_view),
        ])
    }
}

pub struct EndNode;

impl Render for EndNode {
    fn render(
        &self,
        _world: Arc<World>,
        _renderer: &Renderer,
        input_slots: &[Slot],
    ) -> anyhow::Result<Vec<Slot>> {
        Ok(input_slots.to_vec())
    }
}

pub struct RenderGraph {
    graph: StableDiGraph<RenderNode, RenderEdge>,
    node_types: HashMap<TypeId, NodeIndex>,
}

impl Default for RenderGraph {
    fn default() -> Self {
        let mut graph = Self {
            graph: StableDiGraph::new(),
            node_types: HashMap::new(),
        };

        let start_node = graph.add_node(RenderNode::new("Start", StartNode));
        let end_node = graph.add_node(RenderNode::new("End", EndNode));

        graph.add_edge(start_node, 0, end_node, 0);
        graph.add_edge(start_node, 1, end_node, 1);

        graph
    }
}

impl RenderGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: RenderNode) -> NodeIndex {
        let index = self.graph.add_node(node);
        self.node_types
            .insert(self.graph[index].render_type_id(), index);
        index
    }

    pub fn add_edge(
        &mut self,
        from_node: NodeIndex,
        from_slot: usize,
        to_node: NodeIndex,
        to_slot: usize,
    ) {
        // check if edge already exists
        if let Some(edge) = self.graph.find_edge(from_node, to_node) {
            if self.graph[edge].from_slot == from_slot && self.graph[edge].to_slot == to_slot {
                return;
            }
        }

        // check if the to_slot is already occupied
        let mut occupied_edge = None;
        for edge in self.graph.edges_directed(to_node, Direction::Incoming) {
            if edge.weight().to_slot == to_slot {
                occupied_edge = Some(edge.id());
                break;
            }
        }
        if let Some(occupied_edge) = occupied_edge {
            log::debug!(
                "Replacing RenderGraph edge from {:?}:{:?} to {:?}:{:?} with new edge from {:?}:{:?} to {:?}:{:?}",
                self.graph[occupied_edge].from_node,
                self.graph[occupied_edge].from_slot,
                self.graph[occupied_edge].to_node,
                self.graph[occupied_edge].to_slot,
                from_node,
                from_slot,
                to_node,
                to_slot,
            );
            self.graph.remove_edge(occupied_edge);
        }

        self.graph.add_edge(
            from_node,
            to_node,
            RenderEdge {
                from_node,
                from_slot,
                to_node,
                to_slot,
            },
        );
    }

    pub fn remove_edge(&mut self, edge: RenderEdge) {
        while let Some(found) = self.graph.find_edge(edge.from_node, edge.to_node) {
            if self.graph[found].from_slot == edge.from_slot
                && self.graph[found].to_slot == edge.to_slot
            {
                self.graph.remove_edge(found);
            }
        }
    }

    pub fn node_index<T: Render>(&self) -> Option<NodeIndex> {
        self.node_types.get(&TypeId::of::<T>()).copied()
    }

    pub fn prepare(&self, world: Arc<World>, renderer: &Renderer) -> anyhow::Result<()> {
        for node in self.graph.node_indices() {
            let render_node = &self.graph[node];
            render_node.prepare(world.clone(), renderer)?;
        }

        Ok(())
    }

    pub fn render(&self, world: Arc<World>, renderer: &Renderer) -> anyhow::Result<()> {
        let mut output_cache: HashMap<NodeIndex, Vec<Slot>> =
            HashMap::with_capacity(self.graph.node_count());

        let mut bfs = Bfs::new(&self.graph, self.node_index::<StartNode>().unwrap());
        for node in self.graph.externals(Direction::Incoming) {
            if !bfs.stack.contains(&node) {
                bfs.stack.push_back(node);
            }
        }

        while let Some(node) = bfs.next(&self.graph) {
            let render_node = &self.graph[node];

            let mut input_slots = Vec::new();
            let mut edges_incoming = self
                .graph
                .edges_directed(node, Direction::Incoming)
                .collect::<Vec<_>>();
            edges_incoming.sort_by_key(|edge| edge.weight().to_slot);
            for edge in edges_incoming {
                let from_slot = edge.weight().from_slot;
                let output_slots = output_cache.get(&edge.source()).unwrap();
                input_slots.push(output_slots[from_slot].clone());
            }

            let output_slots = render_node.render(world.clone(), renderer, &input_slots)?;
            output_cache.insert(node, output_slots);
        }

        Ok(())
    }

    pub fn write_dot(&self, path: &str) -> anyhow::Result<()> {
        use petgraph::dot::{Config, Dot};

        let dot = Dot::with_config(&self.graph, &[Config::EdgeNoLabel]);
        std::fs::write(path, format!("{:?}", dot))?;

        Ok(())
    }
}

impl Index<NodeIndex> for RenderGraph {
    type Output = RenderNode;

    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.graph[index]
    }
}

impl IndexMut<NodeIndex> for RenderGraph {
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        &mut self.graph[index]
    }
}
