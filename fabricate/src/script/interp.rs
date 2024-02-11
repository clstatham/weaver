use anyhow::ensure;
use rustc_hash::FxHashMap;
use typed_arena::Arena;

use crate::{
    component::{MethodArg, TakesSelf}, prelude::{Data, Entity, LockedWorldHandle, SharedLock}, query::{QueryBuilderAccess, QueryItem}, system::SystemStage
};

use super::{
    parser::{Call, Expr, Query, Scope, Span, SpanExpr, Statement, System, TypedIdent},
    value::{Value, ValueHandle},
    Script,
};

pub struct RuntimeError {
    pub span: Span,
    pub message: String,
}

impl RuntimeError {
    pub fn new(span: Span, message: String) -> Self {
        Self { span, message }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}:{}\n{}",
            self.span.line_no,
            self.span.col_no,
            self.span.line.trim_end()
        )?;
        let spaces = " ".repeat(self.span.col_no.saturating_sub(1));
        writeln!(f, "{}^", spaces)?;
        writeln!(f, "{}", self.message)?;
        Ok(())
    }
}

impl std::fmt::Debug for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for RuntimeError {}

macro_rules! bail {
    ($span:expr, $message:expr) => {
        anyhow::bail!(runtime_error!($span, $message))
    };
}

macro_rules! runtime_error {
    ($span:expr, $message:expr) => {
        anyhow::anyhow!(RuntimeError::new($span.to_owned(), $message.to_owned()))
    };
}

pub(super) struct RuntimeEnv {
    pub world: LockedWorldHandle,
    pub arena: Arena<InterpreterContext>,
}

impl RuntimeEnv {
    pub fn new(world: LockedWorldHandle) -> Self {
        Self {
            world,
            arena: Arena::new(),
        }
    }

    pub fn push_scope(&self, inherit: Option<&InterpreterContext>) -> &mut InterpreterContext {
        if let Some(inherit) = inherit {
            self.arena.alloc(InterpreterContext {
                world: self.world.clone(),
                names: inherit.names.clone(),
                allocations: Vec::new(),
                should_break: None,
                should_return: None,
            })
        } else {
            self.arena.alloc(InterpreterContext {
                world: self.world.clone(),
                names: FxHashMap::default(),
                allocations: Vec::new(),
                should_break: None,
                should_return: None,
            })
        }
    }
}

#[derive(Clone)]
pub(super) struct InterpreterContext {
    pub world: LockedWorldHandle,
    pub names: FxHashMap<String, ValueHandle>,
    pub allocations: Vec<ValueHandle>,
    pub should_break: Option<Option<ValueHandle>>,
    pub should_return: Option<Option<ValueHandle>>,
}

impl Drop for InterpreterContext {
    fn drop(&mut self) {
        for allocation in &self.allocations {
            match allocation {
                ValueHandle::Ref(r) => r.read().despawn(&self.world),
                ValueHandle::Mut(m) => m.read().despawn(&self.world),
            }
        }
    }
}

impl InterpreterContext {
    pub fn alloc_value(&mut self, value: Value) -> ValueHandle {
        let value = ValueHandle::Mut(SharedLock::new(value));
        self.allocations.push(value.clone());
        value
    }

    pub fn forget(&mut self, value: ValueHandle) {
        self.allocations.retain(|v| *v.read() != *value.read());
    }

    pub fn interp_system(
        &mut self,
        env: &RuntimeEnv,
        system: &System,
    ) -> anyhow::Result<ValueHandle> {
        self.interp_block(env, &system.block.statements)
    }

    pub fn interp_block(
        &mut self,
        env: &RuntimeEnv,
        statements: &[Statement],
    ) -> anyhow::Result<ValueHandle> {
        let scope = env.push_scope(Some(self));
        let mut out = scope.alloc_value(Value::Void);
        for statement in statements {
            out = scope.interp_statement(env, statement)?;
            if let Some(retval) = scope.should_return.take() {
                self.should_return = Some(retval.clone());
                if let Some(retval) = retval {
                    out = retval;
                } else {
                    out = scope.alloc_value(Value::Void);
                }
                break;
            }
            if let Some(retval) = scope.should_break.take() {
                self.should_break = Some(retval.clone());
                if let Some(retval) = retval {
                    out = retval;
                } else {
                    out = scope.alloc_value(Value::Void);
                }
                break;
            }
        }
        scope.forget(out.clone());
        Ok(out)
    }

    pub fn interp_statement(
        &mut self,
        env: &RuntimeEnv,
        statement: &Statement,
    ) -> anyhow::Result<ValueHandle> {
        match statement {
            Statement::Expr(expr) => {
                self.interp_expr(env, expr)
            }
            Statement::Break(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_break = Some(retval.clone());
                if let Some(retval) = retval {
                    Ok(retval)
                } else {
                    Ok(self.alloc_value(Value::Void))
                }
            }
            Statement::Return(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_return = Some(retval.clone());
                if let Some(retval) = retval {
                    Ok(retval)
                } else {
                    Ok(self.alloc_value(Value::Void))
                }
            }
            Statement::System(system) => self.interp_system(env, system),
            Statement::Func(_) => todo!(),
            Statement::Component(_) => todo!(),
            Statement::Impl(_) => todo!(),
        }
    }

    pub fn interp_expr(
        &mut self,
        env: &RuntimeEnv,
        expr: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let scope = env.push_scope(Some(self));
        let out = match &expr.expr {
            Expr::IntLiteral(i) => {
                let value = Value::Int(*i);
                let value = scope.alloc_value(value);
                Ok(value)
            }
            Expr::FloatLiteral(f) => {
                let value = Value::Float(*f);
                let value = scope.alloc_value(value);
                Ok(value)
            }
            Expr::StringLiteral(s) => {
                let value = Value::String(s.clone());
                let value = scope.alloc_value(value);
                Ok(value)
            }
            Expr::Ident(ident) => {
                scope.names.get(ident.as_str()).cloned().ok_or_else(|| {
                    runtime_error!(expr.span, format!("Unknown identifier: {}", ident))
                })
            }
            Expr::Infix { op, lhs, rhs } => scope.interp_infix(env, op, lhs, rhs),
            Expr::Decl {
                mutability,
                ident,
                initial_value,
            } => {
                let val = scope.interp_decl(env, *mutability, ident, initial_value)?;
                self.names.insert(ident.as_str().to_string(), val.clone());
                Ok(val)
            },
            Expr::Call(call) => scope.interp_call(env, call),
            Expr::Member { lhs, rhs } => scope.interp_member(env, lhs, rhs),
            Expr::Construct { name, args } => scope.interp_construct(env, name, args),
            Expr::Query(query) => scope.interp_query(env, query),
            Expr::Block(block) => {
                scope.interp_block(env, &block.statements)
            }
            Expr::Loop { condition, block } => scope.interp_loop(env, condition.as_deref(), block),
            Expr::Res {
                mutability,
                ident,
                res,
            } => {
                let val = scope.interp_res(env, *mutability, ident, res)?;
                self.names.insert(ident.as_str().to_string(), val.clone());
                Ok(val)
            },
            _ => todo!(
                "Implement other expressions:\n{}\n{:?}",
                expr.as_str(),
                expr.expr
            ),
        }?;
        scope.forget(out.clone());
        Ok(out)
    }

    pub fn interp_res(
        &mut self,
        _env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        res: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let name = ident.as_str().to_string();

        let res_id = Entity::type_from_name(res.as_str()).ok_or_else(|| {
            runtime_error!(
                res.span,
                format!("Invalid type for resource: {}", res.as_str())
            )
        })?;

        let value = if mutability {
            ValueHandle::Mut(SharedLock::new(Value::Resource(res_id)))
        } else {
            ValueHandle::Ref(SharedLock::new(Value::Resource(res_id)))
        };

        self.names.insert(name, value.to_owned());
        Ok(value)
    }

    pub fn interp_loop(
        &mut self,
        env: &RuntimeEnv,
        condition: Option<&SpanExpr>,
        block: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        match condition {
            Some(condition) => {
                let cond = self.interp_expr(env, condition)?;
                let cond = cond.read();
                if let Value::Bool(false) = &*cond {
                    return Ok(self.alloc_value(Value::Void));
                }
                if let Value::Query(qs) = &*cond {
                    env.world.query(
                        |mut query| {
                            for (_, q) in qs.iter() {
                                match q {
                                    QueryBuilderAccess::Entity => query = query.entity(),
                                    QueryBuilderAccess::Read(id) => {
                                        query = query.read_dynamic(*id).unwrap()
                                    }
                                    QueryBuilderAccess::Write(id) => {
                                        query = query.write_dynamic(*id).unwrap()
                                    }
                                    QueryBuilderAccess::With(id) => {
                                        query = query.with_dynamic(*id).unwrap()
                                    }
                                    QueryBuilderAccess::Without(id) => {
                                        query = query.without_dynamic(*id).unwrap()
                                    }
                                }
                            }

                            query
                        },
                        |query| {
                            for results in query.iter() {
                                let scope = env.push_scope(Some(self));

                                for ((name, _), result) in qs.iter().zip(results.into_inner()) {
                                    let value = match result {
                                        QueryItem::Proxy(p) => {
                                            scope.alloc_value(Value::Data(Data::new_pointer(
                                                p.component.type_id(),
                                                p.component.entity(),
                                            )))
                                        }
                                        QueryItem::ProxyMut(p) => {
                                            scope.alloc_value(Value::Data(Data::new_pointer(
                                                p.component.type_id(),
                                                p.component.entity(),
                                            )))
                                        }
                                        _ => todo!(),
                                    };
                                    scope.names.insert(name.clone(), value);
                                }

                                scope.interp_expr(env, block)?;

                                if scope.should_break.is_some() {
                                    self.should_break = scope.should_break.take();
                                }
                                if scope.should_return.is_some() {
                                    self.should_return = scope.should_return.take();
                                }
                            }

                            Ok::<_, anyhow::Error>(())
                        },
                    )?;
                } else {
                    loop {
                        self.interp_expr(env, block)?;
                        if self.should_break.is_some() {
                            break;
                        }
                        if self.should_return.is_some() {
                            break;
                        }

                        let condition = self.interp_expr(env, condition)?;
                        let condition = condition.read();
                        if let Value::Bool(false) = &*condition {
                            break;
                        }
                    }
                }
            }
            None => loop {
                self.interp_expr(env, block)?;
                if self.should_break.is_some() {
                    break;
                }
                if self.should_return.is_some() {
                    break;
                }
            },
        }
        Ok(self.alloc_value(Value::Void))
    }

    pub fn interp_query(&mut self, env: &RuntimeEnv, query: &Query) -> anyhow::Result<ValueHandle> {
        let names = query
            .components
            .iter()
            .map(|c| c.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let query = query.build_on_world(env.world.clone())?;

        #[allow(clippy::map_identity)]
        let query = names
            .into_iter()
            .zip(query)
            .map(|(n, q)| (n, q))
            .collect::<Vec<_>>();
        Ok(self.alloc_value(Value::Query(query)))
    }

    pub fn interp_construct(
        &mut self,
        _env: &RuntimeEnv,
        _name: &SpanExpr,
        _args: &[(String, SpanExpr)],
    ) -> anyhow::Result<ValueHandle> {
        // let name = name.as_str();
        // let ty = Entity::allocate_type(Some(name));
        // let e = env.world.write().create_entity()?;
        // for (arg_name, arg) in args {
        //     // let value = self.interp_expr(env, arg)?;
        //     // let entity = value.read().entity(env).map_err(|_| {
        //     //     runtime_error!(
        //     //         arg.span,
        //     //         format!("Invalid argument for constructing {}: {}", name, arg_name)
        //     //     )
        //     // })?;
        //     // let mut world = env.world.write();
        //     // let value_ty = entity.type_id().unwrap();
        // }

        todo!("Implement constructing entities");

        // Ok(Value::Data(Data::new_pointer(&ty, &e)).into())
    }

    pub fn interp_call(&mut self, env: &RuntimeEnv, call: &Call) -> anyhow::Result<ValueHandle> {
        match call.name.as_str() {
            "print" => {
                for arg in &call.args {
                    let value = self.interp_expr(env, arg)?;
                    let value = value.display();
                    print!("{}", value);
                }
                println!();
                Ok(self.alloc_value(Value::Void))
            }
            #[cfg(feature = "glam")]
            "vec3" => {
                let x_val = self.interp_expr(env, &call.args[0])?;
                let y_val = self.interp_expr(env, &call.args[1])?;
                let z_val = self.interp_expr(env, &call.args[2])?;
                let x = x_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: x")
                })?;
                x_val.read().despawn(&env.world);
                let y = y_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: y")
                })?;
                y_val.read().despawn(&env.world);
                let z = z_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: z")
                })?;
                z_val.read().despawn(&env.world);

                let v = glam::Vec3::new(x, y, z);
                let data = Data::new_dynamic(&env.world, v);
                let data = self.alloc_value(Value::Data(data));
                Ok(data)
            }
            #[cfg(feature = "glam")]
            "quat" => {
                let axis = self.interp_expr(env, &call.args[0])?;
                let angle_val = self.interp_expr(env, &call.args[1])?;

                let axis_val = axis.read();
                let axis_data = axis_val.as_data().ok_or_else(|| {
                    runtime_error!(call.args[0].span, "Invalid argument for quat: axis")
                })?;
                let axis = axis_data.as_ref::<glam::Vec3>().ok_or_else(|| {
                    runtime_error!(call.args[0].span, "Invalid argument for quat: axis")
                })?;
                axis_val.despawn(&env.world);

                let angle = angle_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.args[1].span, "Invalid argument for quat: angle")
                })?;
                angle_val.read().despawn(&env.world);

                let q = glam::Quat::from_axis_angle(*axis, angle);
                let data = Data::new_dynamic(&env.world, q);
                let data = self.alloc_value(Value::Data(data));
                Ok(data)
            }
            _ => todo!("Implement other calls: {:?}", call),
        }
    }

    pub fn interp_member(
        &mut self,
        env: &RuntimeEnv,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs_val = self.interp_expr(env, lhs)?;

        match &rhs.expr {
            Expr::Ident(_) => bail!(rhs.span, "Invalid member access: accessing member fields of struct components is no longer supported; use a method instead"),
            Expr::Call(call) => {
                let mut args = Vec::new();
                for arg in &call.args {
                    let span = arg.span.clone();
                    let arg = self.interp_expr(env, arg)?;
                    let arg_lock = arg.read();
                    if let Value::Data(data) = &*arg_lock {
                        args.push(MethodArg::Owned(data.to_owned()));
                    } else {
                        let arg = arg_lock.to_owned_data(&env.world).map_err(|_| {
                            runtime_error!(span, "Invalid argument for method call")
                        })?;
                        args.push(MethodArg::Owned(arg));
                    }
                }
                match lhs_val {
                    ValueHandle::Ref(ref lhs_data) => {
                        let lhs_data = lhs_data.read();
                        let data = match &*lhs_data {
                            Value::Resource(res) => {
                                env.world.with_resource_id(*res, |res| res.to_owned().unwrap()).ok_or_else(|| {
                                        runtime_error!(
                                            lhs.span,
                                            format!("Invalid resource: {}", lhs_val.display())
                                        )
                                    })?
                            }
                            Value::Data(data) => data.to_owned(),
                            _ => todo!(),
                        };

                        let vtable = match &data {
                            Data::Dynamic(d) => {
                                d.data().script_vtable(env.world.clone())
                            },
                            Data::Pointer(p) => {
                                // deref one level
                                p.with_deref(&env.world,|d| d.as_dynamic().map(|d| d.data().data().script_vtable(env.world.clone())).ok_or_else(|| {
                                    runtime_error!(rhs.span, "Invalid method call: No such method")
                                }))??
                            }
                        };

                        let result = env.world.defer(move |world, commands| {
                            let mut args = args;
                            if let Some(method) = vtable.get_method(call.name.as_str()) {
                                match method.takes_self {
                                    TakesSelf::None => {},
                                    TakesSelf::RefMut => bail!(call.name.span, "Invalid method call: Method takes self by mutable reference"),
                                    TakesSelf::Ref => {
                                        match &data {
                                            Data::Dynamic(d) => {
                                                let d = world.get(d.entity(), d.type_id()).unwrap();
                                                args.insert(0, MethodArg::Ref(d));
                                            }
                                            Data::Pointer(p) => {
                                                let d = world.storage().find(p.target_type_id(), p.target_entity()).unwrap();
                                                args.insert(0, MethodArg::Ref(d));
                                            }
                                        }
                                    },
                                }
                            } else {
                                bail!(call.name.span, "Invalid method call: No such method")
                            }
    
    
                            let mut result = vtable.call_method(call.name.as_str(), &mut args)?;
    
                            ensure!(
                                result.len() == 1,
                                runtime_error!(
                                    call.name.span,
                                    format!(
                                        "Invalid return value for method call: {}",
                                        call.name.as_str()
                                    )
                                )
                            );
                            let result = result.pop().unwrap();
                            commands.despawn(result.entity());

                            for arg in args {
                                if let MethodArg::Owned(data) = arg {
                                    commands.despawn(data.entity());
                                }
                            }

                            Ok(result)
                        })??;

                        Ok(self.alloc_value(Value::Data(result)))
                    }
                    ValueHandle::Mut(ref lhs_data) => {
                        let lhs_data = lhs_data.write();
                        let data = match &*lhs_data {
                            Value::Resource(res) => {
                                env.world.with_resource_id(*res, |res| res.to_owned().unwrap()).ok_or_else(|| {
                                    runtime_error!(
                                        lhs.span,
                                        format!("Invalid resource: {}", lhs_val.display())
                                    )
                                })?
                            }
                            Value::Data(data) => data.to_owned(),
                            _ => todo!(),
                        };

                        let vtable = match &data {
                            Data::Dynamic(d) => {
                                d.data().script_vtable(env.world.clone())
                            },
                            Data::Pointer(p) => {
                                // deref one level
                                p.with_deref(&env.world, |d| d.as_dynamic().map(|d| d.data().data().script_vtable(env.world.clone())).ok_or_else(|| {
                                    runtime_error!(rhs.span, "Invalid method call: No such method")
                                }))??
                            }
                        };

                        let result = env.world.defer(|world, commands| {
                            let mut args = args;
                            if let Some(method) = vtable.get_method(call.name.as_str()) {
                                match method.takes_self {
                                    TakesSelf::None => {},
                                    TakesSelf::RefMut => {
                                        match &data {
                                            Data::Dynamic(d) => {
                                                let d = world.storage().find_mut(d.type_id(), d.entity()).unwrap();
                                                args.insert(0, MethodArg::Mut(d));
                                            }
                                            Data::Pointer(p) => {
                                                let d = world.storage().find_mut(p.target_type_id(), p.target_entity()).unwrap();
                                                args.insert(0, MethodArg::Mut(d));
                                            }
                                        }
                                    }
                                    TakesSelf::Ref => {
                                        match &data {
                                            Data::Dynamic(d) => {
                                                let d = world.storage().find(d.type_id(), d.entity()).unwrap();
                                                args.insert(0, MethodArg::Ref(d));
                                            }
                                            Data::Pointer(p) => {
                                                let d = world.storage().find(p.target_type_id(), p.target_entity()).unwrap();
                                                args.insert(0, MethodArg::Ref(d));
                                            }
                                        }
                                    },
                                }
                            } else {
                                bail!(call.name.span, "Invalid method call: No such method")
                            }
    
    
                            let mut result = vtable.call_method(call.name.as_str(), &mut args)?;
    
                            ensure!(
                                result.len() == 1,
                                runtime_error!(
                                    call.name.span,
                                    format!(
                                        "Invalid return value for method call: {}",
                                        call.name.as_str()
                                    )
                                )
                            );
                            let result = result.pop().unwrap();
                            commands.despawn(result.entity());

                            for arg in args {
                                if let MethodArg::Owned(data) = arg {
                                    commands.despawn(data.entity());
                                }
                            }

                            Ok(result)
                        })??;

                        Ok(self.alloc_value(Value::Data(result)))
                    }
                }
            }
            _ => bail!(rhs.span, "Invalid member access"),
        }
    }

    pub fn interp_infix(
        &mut self,
        env: &RuntimeEnv,
        op: &str,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs_val = self.interp_expr(env, lhs)?;
        let rhs_val = self.interp_expr(env, rhs)?;
        let out = lhs_val.infix(op, &rhs_val, self)?;
        Ok(out)
    }

    pub fn interp_decl(
        &mut self,
        env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        initial_value: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let value = self.interp_expr(env, initial_value)?;
        let name = ident.as_str().to_string();
        let value = if mutability {
            match value {
                ValueHandle::Ref(ref r) => ValueHandle::Mut(r.to_owned()),
                ValueHandle::Mut(ref m) => ValueHandle::Mut(m.to_owned()),
            }
        } else {
            match value {
                ValueHandle::Ref(ref r) => ValueHandle::Ref(r.to_owned()),
                ValueHandle::Mut(ref m) => ValueHandle::Ref(m.to_owned()),
            }
        };
        self.names.insert(name, value.to_owned());
        Ok(value)
    }
}


pub trait BuildOnWorld {
    type Output;

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<Self::Output>;
}

impl BuildOnWorld for Script {
    type Output = ();

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<()> {
        for scope in &self.scopes {
            match scope {
                Scope::System(ref system) => {
                    let system_clone = system.clone();
                    let run_fn = move |world: LockedWorldHandle| {
                        let env = RuntimeEnv::new(world.clone());
                        let ctx = env.push_scope(None);

                        ctx.interp_system(&env, &system_clone).unwrap();
                    };

                    let tag = system.tag.clone();

                    let stage = match tag.as_deref() {
                        Some("@startup") => SystemStage::Startup,
                        Some("@pre_update") => SystemStage::PreUpdate,
                        Some("@update") => SystemStage::Update,
                        Some("@post_update") => SystemStage::PostUpdate,
                        Some("@shutdown") => SystemStage::Shutdown,
                        Some(tag) => todo!("Unknown system tag: {}", tag),
                        None => SystemStage::Update,
                    };

                    world.add_system(stage, run_fn)?;
                }

                Scope::Component(_component) => {
                    todo!("Implement component registration")
                }
                Scope::Func(_) => {}
                Scope::Impl(_) => {}
                Scope::Program(_) => unreachable!(),
            }
        }

        Ok(())
    }
}

impl BuildOnWorld for Query {
    type Output = Vec<QueryBuilderAccess>;

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<Vec<QueryBuilderAccess>> {
        let mut query = Vec::new();
        for component in &self.components {
            let id = component.build_on_world(world.clone())?;
            if component.mutability {
                query.push(QueryBuilderAccess::Write(id));
            } else {
                query.push(QueryBuilderAccess::Read(id));
            }
        }
        for with in &self.with {
            let id = with.as_str().to_string().build_on_world(world.clone())?;
            query.push(QueryBuilderAccess::With(id));
        }
        for without in &self.without {
            let id = without.as_str().to_string().build_on_world(world.clone())?;
            query.push(QueryBuilderAccess::Without(id));
        }
        Ok(query)
    }
}

impl BuildOnWorld for TypedIdent {
    type Output = Entity;

    fn build_on_world(&self, _world: LockedWorldHandle) -> anyhow::Result<Entity> {
        Ok(Entity::allocate_type(Some(self.ty.as_str())))
    }
}

impl BuildOnWorld for String {
    type Output = Entity;

    fn build_on_world(&self, _world: LockedWorldHandle) -> anyhow::Result<Entity> {
        Ok(Entity::allocate_type(Some(self.as_str())))
    }
}
