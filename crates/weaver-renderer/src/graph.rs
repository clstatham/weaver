use petgraph::prelude::*;
use weaver_ecs::world::World;
use weaver_util::lock::Lock;

use crate::{target::RenderTarget, Renderer};

pub type RenderEdge = ();

pub trait Render: 'static {
    #[allow(unused_variables)]
    fn prepare(&mut self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &self,
        world: &World,
        renderer: &Renderer,
        target: &RenderTarget,
    ) -> anyhow::Result<()>;
}

pub struct RenderNode {
    name: String,
    render: Lock<Box<dyn Render>>,
    target: RenderTarget,
}

impl RenderNode {
    pub fn new(name: &str, render: impl Render, target: RenderTarget) -> Self {
        Self {
            name: name.to_string(),
            render: Lock::new(Box::new(render)),
            target,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        self.render.write().render(world, renderer, &self.target)
    }
}

#[derive(Default)]
pub struct RenderGraph {
    graph: StableDiGraph<RenderNode, RenderEdge>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: RenderNode) -> NodeIndex {
        self.graph.add_node(node)
    }

    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex) {
        self.graph.add_edge(from, to, ());
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        for node in self.graph.node_indices() {
            let mut render_node = self.graph[node].render.write();
            render_node.prepare(world, renderer)?;
        }

        Ok(())
    }

    pub fn render(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        let mut visited = vec![false; self.graph.node_count()];
        let mut stack = Vec::new();

        for node in self.graph.node_indices() {
            self.visit(node, &mut visited, &mut stack);
        }

        for node in stack {
            let render_node = &self.graph[node];
            render_node.render(world, renderer)?;
        }

        Ok(())
    }

    fn visit(&self, node: NodeIndex, visited: &mut Vec<bool>, stack: &mut Vec<NodeIndex>) {
        if visited[node.index()] {
            return;
        }

        visited[node.index()] = true;

        for child in self.graph.neighbors(node) {
            self.visit(child, visited, stack);
        }

        stack.push(node);
    }
}
