use petgraph::prelude::*;
use weaver_ecs::scene::Scene;

use crate::{target::RenderTarget, Renderer};

pub type RenderEdge = ();

pub trait Render: 'static {
    fn render(
        &self,
        scene: &Scene,
        renderer: &Renderer,
        target: &RenderTarget,
    ) -> anyhow::Result<()>;
}

pub struct RenderNode {
    name: String,
    render: Box<dyn Render>,
}

impl RenderNode {
    pub fn new(name: &str, render: impl Render) -> Self {
        Self {
            name: name.to_string(),
            render: Box::new(render),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render(
        &self,
        scene: &Scene,
        renderer: &Renderer,
        target: &RenderTarget,
    ) -> anyhow::Result<()> {
        self.render.render(scene, renderer, target)
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

    pub fn render(
        &self,
        scene: &Scene,
        renderer: &Renderer,
        target: &RenderTarget,
    ) -> anyhow::Result<()> {
        let mut visited = vec![false; self.graph.node_count()];
        let mut stack = Vec::new();

        for node in self.graph.node_indices() {
            self.visit(node, &mut visited, &mut stack);
        }

        for node in stack {
            let render_node = &self.graph[node];
            render_node.render(scene, renderer, target)?;
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
