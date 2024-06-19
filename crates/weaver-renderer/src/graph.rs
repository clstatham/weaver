use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    sync::Arc,
};

use petgraph::{prelude::*, visit::Topo};
use weaver_ecs::{
    entity::Entity,
    prelude::Resource,
    query::{Query, QueryFetch, QueryFilter},
    world::World,
};
use weaver_util::{
    lock::{Lock, Read, Write},
    prelude::{anyhow, bail, impl_downcast, DowncastSync, Result},
    TypeIdMap,
};

use crate::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawFnId(u32);

impl DrawFnId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u32 {
        self.0
    }
}

pub trait DrawItem: Send + Sync + 'static {
    fn entity(&self) -> Entity;
    fn draw_fn(&self) -> DrawFnId;
}

pub trait DrawFn<T: DrawItem>: 'static + Send + Sync {
    #[allow(unused_variables)]
    fn prepare(&mut self, render_world: &World) -> Result<()> {
        Ok(())
    }

    fn draw(
        &mut self,
        render_world: &World,
        render_pass: wgpu::RenderPass<'_>,
        view_entity: Entity,
        item: &T,
    ) -> Result<()>;
}

pub struct DrawFunctionsInner<T: DrawItem> {
    functions: Vec<Box<dyn DrawFn<T>>>,
    indices: TypeIdMap<DrawFnId>,
}

impl<T: DrawItem> DrawFunctionsInner<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            indices: TypeIdMap::default(),
        }
    }

    pub fn add<F: DrawFn<T> + 'static>(&mut self, function: F) -> DrawFnId {
        let id = DrawFnId::new(self.functions.len() as u32);
        self.functions.push(Box::new(function));
        self.indices.insert(TypeId::of::<F>(), id);
        id
    }

    pub fn get(&self, id: DrawFnId) -> Option<&dyn DrawFn<T>> {
        self.functions.get(id.id() as usize).map(|f| f.as_ref())
    }

    pub fn get_mut(&mut self, id: DrawFnId) -> Option<&mut dyn DrawFn<T>> {
        self.functions.get_mut(id.id() as usize).map(|f| f.as_mut())
    }

    pub fn get_id<F: DrawFn<T>>(&self) -> Option<DrawFnId> {
        self.indices.get(&TypeId::of::<F>()).copied()
    }
}

#[derive(Resource)]
pub struct DrawFunctions<T: DrawItem> {
    inner: Lock<DrawFunctionsInner<T>>,
}

impl<T: DrawItem> DrawFunctions<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inner: Lock::new(DrawFunctionsInner::new()),
        }
    }

    pub fn read(&self) -> Read<'_, DrawFunctionsInner<T>> {
        self.inner.read()
    }

    pub fn write(&self) -> Write<'_, DrawFunctionsInner<T>> {
        self.inner.write()
    }
}

#[derive(Clone, Copy)]
pub struct RenderId {
    pub id: TypeId,
    pub name: &'static str,
}

impl RenderId {
    pub fn of<T: RenderLabel>(label: T) -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: label.name(),
        }
    }
}

impl Debug for RenderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RenderId").field(&self.name).finish()
    }
}

impl PartialEq for RenderId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderId {}

impl Hash for RenderId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

pub trait RenderLabel: Any + Debug + Clone + Copy {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderEdge {
    Slot {
        from_node: RenderId,
        from_slot: usize,
        to_node: RenderId,
        to_slot: usize,
    },
    Node {
        from_node: RenderId,
        to_node: RenderId,
    },
}

impl RenderEdge {
    pub fn get_from_node(&self) -> RenderId {
        match self {
            Self::Slot { from_node, .. } => *from_node,
            Self::Node { from_node, .. } => *from_node,
        }
    }

    pub fn get_to_node(&self) -> RenderId {
        match self {
            Self::Slot { to_node, .. } => *to_node,
            Self::Node { to_node, .. } => *to_node,
        }
    }
}

#[derive(Clone)]
pub enum Slot {
    Buffer(Arc<wgpu::Buffer>),
    Texture(Arc<wgpu::TextureView>),
    BindGroup(Arc<wgpu::BindGroup>),
}

#[derive(Debug, Clone, Copy)]
pub enum SlotType {
    Buffer,
    Texture,
    BindGroup,
}

pub trait RenderNode: DowncastSync {
    fn input_slots(&self) -> Vec<SlotType> {
        Vec::new()
    }
    fn output_slots(&self) -> Vec<SlotType> {
        Vec::new()
    }

    #[allow(unused)]
    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        Ok(())
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
    ) -> Result<()>;
}
impl_downcast!(RenderNode);

pub trait ViewNode: Send + Sync + 'static {
    type ViewQueryFetch: QueryFetch;
    type ViewQueryFilter: QueryFilter;

    fn input_slots(&self) -> Vec<SlotType> {
        Vec::new()
    }
    fn output_slots(&self) -> Vec<SlotType> {
        Vec::new()
    }

    #[allow(unused)]
    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        Ok(())
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        view_query: &<Self::ViewQueryFetch as QueryFetch>::Fetch,
    ) -> Result<()>;
}

pub struct ViewNodeRunner<T: ViewNode> {
    pub node: T,
    pub view_query: Query<T::ViewQueryFetch, T::ViewQueryFilter>,
}

impl<T: ViewNode> ViewNodeRunner<T> {
    pub fn new(node: T, render_world: &World) -> Self {
        Self {
            node,
            view_query: render_world.query_filtered(),
        }
    }
}

impl<T: ViewNode> RenderNode for ViewNodeRunner<T> {
    fn input_slots(&self) -> Vec<SlotType> {
        self.node.input_slots()
    }

    fn output_slots(&self) -> Vec<SlotType> {
        self.node.output_slots()
    }

    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        self.view_query = render_world.query_filtered();
        self.node.prepare(render_world)
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
    ) -> Result<()> {
        let Some(view_query) = self.view_query.get(graph_ctx.view_entity) else {
            log::warn!("View query not found");
            return Ok(());
        };

        self.node
            .run(render_world, graph_ctx, render_ctx, &view_query)
    }
}

pub struct RenderNodeState {
    pub node_id: RenderId,
    pub node: Box<dyn RenderNode>,
    pub node_type_name: &'static str,
    pub input_slots: Vec<SlotType>,
    pub output_slots: Vec<SlotType>,
    pub input_edges: Vec<RenderEdge>,
    pub output_edges: Vec<RenderEdge>,
}

impl RenderNodeState {
    pub fn new<T: RenderNode>(label: impl RenderLabel, node: T) -> Self {
        Self {
            node_id: RenderId::of(label),
            input_slots: node.input_slots(),
            output_slots: node.output_slots(),
            node_type_name: std::any::type_name::<T>(),
            node: Box::new(node),
            input_edges: Vec::new(),
            output_edges: Vec::new(),
        }
    }

    pub fn node<T: RenderNode>(&self) -> Option<&T> {
        self.node.downcast_ref::<T>()
    }

    pub fn node_mut<T: RenderNode>(&mut self) -> Option<&mut T> {
        self.node.downcast_mut::<T>()
    }

    pub fn has_input_edge(&self, edge: RenderEdge) -> bool {
        self.input_edges.contains(&edge)
    }

    pub fn has_output_edge(&self, edge: RenderEdge) -> bool {
        self.output_edges.contains(&edge)
    }

    pub fn add_input_edge(&mut self, edge: RenderEdge) -> Result<()> {
        if self.has_input_edge(edge) {
            bail!("Input edge already exists");
        }
        self.input_edges.push(edge);
        Ok(())
    }

    pub fn add_output_edge(&mut self, edge: RenderEdge) -> Result<()> {
        if self.has_output_edge(edge) {
            bail!("Output edge already exists");
        }
        self.output_edges.push(edge);
        Ok(())
    }

    pub fn remove_input_edge(&mut self, edge: RenderEdge) -> bool {
        if self.has_input_edge(edge) {
            self.input_edges.retain(|e| e != &edge);
            true
        } else {
            false
        }
    }

    pub fn remove_output_edge(&mut self, edge: RenderEdge) -> bool {
        if self.has_output_edge(edge) {
            self.output_edges.retain(|e| e != &edge);
            true
        } else {
            false
        }
    }

    pub fn get_input_slot_edge(&self, index: usize) -> Option<&RenderEdge> {
        self.input_edges.iter().find(|e| match e {
            RenderEdge::Slot { to_slot, .. } => *to_slot == index,
            _ => false,
        })
    }

    pub fn get_output_slot_edge(&self, index: usize) -> Option<&RenderEdge> {
        self.output_edges.iter().find(|e| match e {
            RenderEdge::Slot { from_slot, .. } => *from_slot == index,
            _ => false,
        })
    }

    pub fn validate_input_edges(&self) -> Result<()> {
        for i in 0..self.input_slots.len() {
            if self.get_input_slot_edge(i).is_none() {
                bail!("Missing input edge for slot {}", i);
            }
        }

        Ok(())
    }

    pub fn validate_output_edges(&self) -> Result<()> {
        for i in 0..self.output_slots.len() {
            if self.get_output_slot_edge(i).is_none() {
                bail!("Missing output edge for slot {}", i);
            }
        }

        Ok(())
    }
}

pub struct RenderCtx<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    command_encoder: Option<wgpu::CommandEncoder>,
}

impl<'a> RenderCtx<'a> {
    pub fn new(device: &'a wgpu::Device, queue: &'a wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            command_encoder: None,
        }
    }

    pub fn command_encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.command_encoder.get_or_insert_with(|| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("RenderCtx Command Encoder"),
                })
        })
    }

    pub fn end(&mut self) -> Option<wgpu::CommandBuffer> {
        self.command_encoder.take().map(|encoder| encoder.finish())
    }

    pub fn begin_render_pass<'b, 't: 'b>(
        &'b mut self,
        pass_descriptor: &wgpu::RenderPassDescriptor<'t, '_>,
    ) -> wgpu::RenderPass<'b> {
        self.command_encoder().begin_render_pass(pass_descriptor)
    }
}

pub struct RenderGraphCtx<'a> {
    pub render_graph: &'a RenderGraph,
    pub node: &'a RenderNodeState,
    pub inputs: &'a [Slot],
    pub outputs: &'a mut [Option<Slot>],
    pub view_entity: Entity,
}

impl<'a> RenderGraphCtx<'a> {
    pub fn input(&self, index: usize) -> &Slot {
        &self.inputs[index]
    }

    pub fn output(&mut self, index: usize) -> &mut Option<Slot> {
        &mut self.outputs[index]
    }

    pub fn set_output(&mut self, index: usize, slot: Slot) {
        self.outputs[index] = Some(slot);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphInputLabel;
impl RenderLabel for GraphInputLabel {}

pub struct GraphInputNode {
    pub inputs: Vec<SlotType>,
}

impl RenderNode for GraphInputNode {
    fn input_slots(&self) -> Vec<SlotType> {
        self.inputs.clone()
    }

    fn output_slots(&self) -> Vec<SlotType> {
        self.inputs.clone()
    }

    fn run(
        &self,
        _render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        _render_ctx: &mut RenderCtx,
    ) -> Result<()> {
        for i in 0..self.inputs.len() {
            graph_ctx.set_output(i, graph_ctx.input(i).clone());
        }

        Ok(())
    }
}

pub struct RenderGraph {
    graph: StableDiGraph<RenderNodeState, ()>,
    node_ids: HashMap<RenderId, NodeIndex>,
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self {
            graph: StableDiGraph::new(),
            node_ids: HashMap::new(),
        }
    }
}

impl RenderGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_node(&self, label: impl RenderLabel) -> bool {
        self.node_ids.contains_key(&RenderId::of(label))
    }

    pub fn get_node_state(&self, label: impl RenderLabel) -> Option<&RenderNodeState> {
        self.node_ids
            .get(&RenderId::of(label))
            .map(|node_id| &self.graph[*node_id])
    }

    pub fn get_node_state_mut(&mut self, label: impl RenderLabel) -> Option<&mut RenderNodeState> {
        self.node_ids
            .get(&RenderId::of(label))
            .map(|node_id| &mut self.graph[*node_id])
    }

    pub fn get_node<T: RenderNode>(&self, label: impl RenderLabel) -> Option<&T> {
        self.get_node_state(label)?.node::<T>()
    }

    pub fn get_node_mut<T: RenderNode>(&mut self, label: impl RenderLabel) -> Option<&mut T> {
        self.get_node_state_mut(label)?.node_mut::<T>()
    }

    pub fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        let mut search = Topo::new(&self.graph);

        while let Some(node) = search.next(&self.graph) {
            let render_node = &mut self.graph[node];
            render_node.node.prepare(render_world)?;
        }

        Ok(())
    }

    pub fn set_inputs(&mut self, inputs: Vec<SlotType>) -> Result<()> {
        if self.get_node_state(GraphInputLabel).is_some() {
            bail!("Graph inputs already set");
        }

        self.add_node(GraphInputLabel, GraphInputNode { inputs })?;
        Ok(())
    }

    pub fn add_node(&mut self, label: impl RenderLabel, node: impl RenderNode) -> Result<()> {
        let node_state = RenderNodeState::new(label, node);
        let node_id = node_state.node_id;

        if self.node_ids.contains_key(&node_id) {
            bail!("Node already exists");
        }

        let node_index = self.graph.add_node(node_state);

        self.node_ids.insert(node_id, node_index);
        Ok(())
    }

    pub fn remove_node(&mut self, label: impl RenderLabel) -> Result<()> {
        let node_id = RenderId::of(label);
        let Some(node_index) = self.node_ids.remove(&node_id) else {
            return Ok(());
        };

        let node = self.graph.remove_node(node_index).unwrap();
        for edge in node.input_edges {
            let from_node = self.node_ids.get(&edge.get_from_node()).unwrap();
            self.graph[*from_node].remove_output_edge(edge);
        }
        for edge in node.output_edges {
            let to_node = self.node_ids.get(&edge.get_to_node()).unwrap();
            self.graph[*to_node].remove_input_edge(edge);
        }

        Ok(())
    }

    pub fn try_add_node_edge<F: RenderLabel, T: RenderLabel>(
        &mut self,
        from: F,
        to: T,
    ) -> Result<()> {
        let from_id = RenderId::of(from);
        let to_id = RenderId::of(to);

        let from_node = self
            .node_ids
            .get(&from_id)
            .ok_or_else(|| anyhow!("from_node not found: {:?}", from_id))?;
        let to_node = self
            .node_ids
            .get(&to_id)
            .ok_or_else(|| anyhow!("to_node not found: {:?}", to_id))?;

        if self.graph.find_edge(*from_node, *to_node).is_some() {
            bail!("Edge already exists");
        }

        let edge = RenderEdge::Node {
            from_node: from_id,
            to_node: to_id,
        };

        {
            let from_node = &mut self.graph[*from_node];

            from_node.add_output_edge(edge)?;
        }
        {
            let to_node = &mut self.graph[*to_node];
            to_node.add_input_edge(edge)?;
        }

        self.graph.add_edge(*from_node, *to_node, ());

        Ok(())
    }

    pub fn try_add_slot_edge(
        &mut self,
        from: impl RenderLabel,
        from_slot: usize,
        to: impl RenderLabel,
        to_slot: usize,
    ) -> Result<()> {
        let from_id = RenderId::of(from);
        let to_id = RenderId::of(to);

        let from_node = self
            .node_ids
            .get(&from_id)
            .ok_or_else(|| anyhow!("Node not found: {:?}", from_id))?;
        let to_node = self
            .node_ids
            .get(&to_id)
            .ok_or_else(|| anyhow!("Node not found: {:?}", to_id))?;

        if self.graph.find_edge(*from_node, *to_node).is_some() {
            bail!("Edge already exists");
        }

        let edge = RenderEdge::Slot {
            from_node: from_id,
            from_slot,
            to_node: to_id,
            to_slot,
        };

        {
            let from_node = &mut self.graph[*from_node];
            from_node.add_output_edge(edge)?;
        }
        {
            let to_node = &mut self.graph[*to_node];
            to_node.add_input_edge(edge)?;
        }

        self.graph.add_edge(*from_node, *to_node, ());

        Ok(())
    }

    pub fn try_remove_node_edge(
        &mut self,
        from: impl RenderLabel,
        to: impl RenderLabel,
    ) -> Result<()> {
        let from_id = RenderId::of(from);
        let to_id = RenderId::of(to);

        let from_node = self
            .node_ids
            .get(&from_id)
            .ok_or_else(|| anyhow!("Node not found: {:?}", from_id))?;
        let to_node = self
            .node_ids
            .get(&to_id)
            .ok_or_else(|| anyhow!("Node not found: {:?}", to_id))?;

        let edge = RenderEdge::Node {
            from_node: from_id,
            to_node: to_id,
        };

        {
            let from_node = &mut self.graph[*from_node];
            from_node.remove_output_edge(edge);
        }
        {
            let to_node = &mut self.graph[*to_node];
            to_node.remove_input_edge(edge);
        }

        let edge_index = self.graph.find_edge(*from_node, *to_node).unwrap();
        self.graph.remove_edge(edge_index);

        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        for node_index in self.graph.node_indices() {
            let node = &self.graph[node_index];
            node.validate_input_edges()?;
            node.validate_output_edges()?;

            for edge in &node.input_edges {
                let from_node_index = self.node_ids.get(&edge.get_from_node()).unwrap();
                let from_node = &self.graph[*from_node_index];
                if !from_node.has_output_edge(*edge) {
                    bail!("Missing output edge");
                }

                if self.graph.find_edge(*from_node_index, node_index).is_none() {
                    bail!("Missing graph edge");
                }
            }

            for edge in &node.output_edges {
                let to_node_index = self.node_ids.get(&edge.get_to_node()).unwrap();
                let to_node = &self.graph[*to_node_index];
                if !to_node.has_input_edge(*edge) {
                    bail!("Missing input edge");
                }

                if self.graph.find_edge(node_index, *to_node_index).is_none() {
                    bail!("Missing graph edge");
                }
            }
        }

        Ok(())
    }

    pub fn run(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut Renderer,
        render_world: &World,
        view_entity: Entity,
    ) -> Result<()> {
        let mut search = Topo::new(&self.graph);

        let mut output_cache: HashMap<RenderId, Vec<Slot>> = HashMap::new();

        let mut render_ctx = RenderCtx::new(device, queue);

        while let Some(node) = search.next(&self.graph) {
            let node_state = &self.graph[node];
            if output_cache.contains_key(&node_state.node_id) {
                continue;
            }

            let mut inputs = Vec::new();
            let input_slot_edges = node_state
                .input_edges
                .iter()
                .filter(|edge| matches!(edge, RenderEdge::Slot { .. }))
                .collect::<Vec<_>>();
            let mut maybe_inputs = vec![None; input_slot_edges.len()];
            for edge in &input_slot_edges {
                let RenderEdge::Slot {
                    from_node,
                    from_slot,
                    to_node,
                    to_slot,
                } = edge
                else {
                    continue;
                };

                if *to_node != node_state.node_id {
                    bail!("Invalid edge");
                }

                let from_node = self.node_ids.get(from_node).unwrap();
                let from_node_state = &self.graph[*from_node];
                let output = output_cache
                    .get(&from_node_state.node_id)
                    .ok_or_else(|| anyhow!("Missing output cache"))?
                    .get(*from_slot)
                    .ok_or_else(|| anyhow!("Missing output"))?
                    .clone();

                maybe_inputs[*to_slot] = Some(output);
            }

            for maybe_input in maybe_inputs {
                inputs.push(maybe_input.ok_or_else(|| anyhow!("Missing input"))?);
            }

            let mut maybe_outputs = vec![None; node_state.output_slots.len()];
            let mut graph_ctx = RenderGraphCtx {
                render_graph: self,
                node: node_state,
                inputs: &inputs,
                outputs: &mut maybe_outputs,
                view_entity,
            };

            node_state
                .node
                .run(render_world, &mut graph_ctx, &mut render_ctx)?;

            let mut outputs = Vec::new();
            for maybe_output in maybe_outputs {
                outputs.push(maybe_output.ok_or_else(|| anyhow!("Missing output"))?);
            }

            output_cache.insert(node_state.node_id, outputs);
        }

        if let Some(commands) = render_ctx.end() {
            renderer.enqueue_command_buffer(commands);
        }

        Ok(())
    }
}
