use std::fmt::Debug;

use anyhow::{bail, ensure, Result};
use pest::{
    iterators::{Pair, Pairs},
    pratt_parser::{Assoc, Op, PrattParser},
    Parser,
};
use pest_derive::Parser;

#[derive(Debug, Clone)]
pub struct TypedIdent {
    pub mutability: bool,
    pub name: SpanExpr,
    pub ty: SpanExpr,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: SpanExpr,
    pub fields: Vec<TypedIdent>,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub name: Box<SpanExpr>,
    pub args: Vec<SpanExpr>,
}

#[derive(Clone)]
pub enum Expr {
    Ident(String),
    Decl {
        mutability: bool,
        ident: Box<SpanExpr>,
        initial_value: Box<SpanExpr>,
    },
    Construct {
        name: Box<SpanExpr>,
        args: Vec<(String, SpanExpr)>,
    },
    IntLiteral(i64),
    FloatLiteral(f32),
    StringLiteral(String),
    Call(Call),
    Block(Block),
    Type(String),
    Member {
        lhs: Box<SpanExpr>,
        rhs: Box<SpanExpr>,
    },
    Infix {
        op: String,
        lhs: Box<SpanExpr>,
        rhs: Box<SpanExpr>,
    },
    Prefix {
        op: String,
        rhs: Box<SpanExpr>,
    },
    If {
        condition: Box<SpanExpr>,
        then_block: Box<SpanExpr>,
        elif_blocks: Vec<(Box<SpanExpr>, Box<SpanExpr>)>,
        else_block: Option<Box<SpanExpr>>,
    },
    Loop {
        condition: Option<Box<SpanExpr>>,
        block: Box<SpanExpr>,
    },
    Query(Query),
    Res {
        mutability: bool,
        ident: Box<SpanExpr>,
        res: Box<SpanExpr>,
    },
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block(_) => write!(f, "Block"),
            Self::Ident(ident) => write!(f, "Ident({})", ident),
            Self::Decl {
                mutability,
                ident,
                initial_value: _,
            } => write!(
                f,
                "Decl {{ mutability: {}, ident: {:?}}}",
                mutability, ident
            ),
            Self::Construct { name, args } => {
                write!(f, "Construct {{ name: {:?}, args: {:?}}}", name, args)
            }
            Self::IntLiteral(int) => write!(f, "IntLiteral({})", int),
            Self::FloatLiteral(float) => write!(f, "FloatLiteral({})", float),
            Self::StringLiteral(string) => write!(f, "StringLiteral({})", string),
            Self::Call(call) => write!(f, "Call({:?})", call),
            Self::Type(ty) => write!(f, "Type({})", ty),
            Self::Member { lhs, rhs } => write!(f, "Member({:?}, {:?})", lhs, rhs),
            Self::Infix { op, lhs, rhs } => write!(f, "Infix({} {:?} {:?})", op, lhs, rhs),
            Self::Prefix { op, rhs } => write!(f, "Prefix({} {:?})", op, rhs),
            Self::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => write!(
                f,
                "If {{ condition: {:?}, then_block: {:?}, elif_blocks: {:?}, else_block: {:?}}}",
                condition, then_block, elif_blocks, else_block
            ),
            Self::Loop { condition, block } => write!(
                f,
                "Loop {{ condition: {:?}, block: {:?}}}",
                condition, block
            ),
            Self::Query(query) => write!(f, "Query({:?})", query),
            Self::Res {
                mutability,
                ident,
                res,
            } => write!(
                f,
                "Res {{ mutability: {}, ident: {:?}, res: {:?}}}",
                mutability, ident, res
            ),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Span {
    pub line_no: usize,
    pub col_no: usize,
    pub line: String,
    pub fragment: String,
}

#[derive(Debug, Clone)]
pub struct SpanExpr {
    pub expr: Expr,
    pub span: Span,
}

impl SpanExpr {
    pub fn new(pair: Pair<'_, Rule>, expr: Expr) -> Self {
        let span = pair.as_span();
        let line_no = span.start_pos().line_col().0;
        let col_no = span.start_pos().line_col().1;
        let span = span.lines_span().next().unwrap();

        Self {
            expr,
            span: Span {
                line_no,
                col_no,
                line: span.as_str().to_string(),
                fragment: pair.as_str().to_string(),
            },
        }
    }

    pub fn as_str(&self) -> &str {
        &self.span.fragment
    }
}

impl PartialEq for SpanExpr {
    fn eq(&self, other: &Self) -> bool {
        self.span.line == other.span.line
    }
}

#[derive(Debug, Clone)]
pub struct Func {
    pub name: Box<SpanExpr>,
    pub params: Vec<TypedIdent>,
    pub ret_type: Option<Box<SpanExpr>>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct Impl {
    pub ty: Box<SpanExpr>,
    pub funcs: Vec<Func>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Component(Component),
    System(System),
    Func(Func),
    Expr(SpanExpr),
    Break(Option<SpanExpr>),
    Return(Option<SpanExpr>),
    Impl(Impl),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub name: Box<SpanExpr>,
    pub components: Vec<TypedIdent>,
    pub with: Vec<SpanExpr>,
    pub without: Vec<SpanExpr>,
}

#[derive(Debug, Clone)]
pub struct System {
    pub tag: Option<String>,
    pub name: Box<SpanExpr>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub enum Scope {
    Program(Vec<Scope>),
    Component(Component),
    System(System),
    Func(Func),
    Impl(Impl),
}

impl Default for Scope {
    fn default() -> Self {
        Scope::Program(Vec::new())
    }
}

#[derive(Parser)]
#[grammar = "script/loom.pest"]
pub struct LoomParser {
    top_scope: Scope,
}

impl LoomParser {
    pub fn new() -> Self {
        Self {
            top_scope: Scope::default(),
        }
    }

    pub fn finish(self) -> Result<Vec<Scope>> {
        match self.top_scope {
            Scope::Program(scopes) => Ok(scopes),
            _ => bail!("Unexpected scope"),
        }
    }

    pub fn parse_script(&mut self, script: &str) -> anyhow::Result<()> {
        let mut pairs = LoomParser::parse(Rule::program, script)?;

        let program = pairs.next().unwrap();
        ensure!(program.as_rule() == Rule::program);

        let start = program.into_inner().next().unwrap();
        ensure!(start.as_rule() == Rule::statements);
        for pair in start.into_inner() {
            match pair.as_rule() {
                Rule::statements => {
                    let stmts = self.parse_statements(pair)?;
                    for stmt in stmts {
                        self.push_statement(stmt)?;
                    }
                }
                Rule::statement => {
                    let stmt = self.parse_statement(pair)?;
                    self.push_statement(stmt)?;
                }
                _ => bail!("Unexpected rule: {:?}", pair.as_rule()),
            }
        }

        Ok(())
    }

    fn push_statement(&mut self, stmt: Statement) -> Result<()> {
        match &mut self.top_scope {
            Scope::Program(stmts) => match stmt {
                Statement::Component(component) => {
                    stmts.push(Scope::Component(component));
                }
                Statement::System(system) => {
                    stmts.push(Scope::System(system));
                }
                Statement::Func(func) => {
                    stmts.push(Scope::Func(func));
                }
                Statement::Impl(impl_) => {
                    stmts.push(Scope::Impl(impl_));
                }
                stmt => bail!("Unexpected statement: {:?}", stmt),
            },
            _ => bail!("Unexpected scope"),
        }
        Ok(())
    }

    fn parse_statements(&mut self, pair: Pair<Rule>) -> Result<Vec<Statement>> {
        ensure!(pair.as_rule() == Rule::statements);
        let mut statements = Vec::new();
        for stmt in pair.into_inner() {
            let stmt = match stmt.as_rule() {
                Rule::statement => self.parse_statement(stmt)?,
                _ => bail!("Unexpected rule: {:?}", stmt.as_rule()),
            };
            statements.push(stmt);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self, pair: Pair<Rule>) -> Result<Statement> {
        ensure!(pair.as_rule() == Rule::statement);
        let mut inner = pair.into_inner();
        let first = inner.next().unwrap();

        match first.as_rule() {
            Rule::component_stmt => self.parse_component_stmt(first),
            Rule::system_stmt => self.parse_system_stmt(first),
            Rule::func_stmt => self.parse_func_stmt(first),
            Rule::expr => Ok(Statement::Expr(self.parse_expr(first)?)),
            Rule::impl_stmt => self.parse_impl_stmt(first),
            Rule::break_stmt => {
                let mut inner = first.into_inner();
                if let Some(expr) = inner.next() {
                    let expr = self.parse_expr(expr)?;
                    Ok(Statement::Break(Some(expr)))
                } else {
                    Ok(Statement::Break(None))
                }
            }
            Rule::return_stmt => {
                let mut inner = first.into_inner();
                if let Some(expr) = inner.next() {
                    let expr = self.parse_expr(expr)?;
                    Ok(Statement::Return(Some(expr)))
                } else {
                    Ok(Statement::Return(None))
                }
            }
            _ => bail!("Unexpected rule: {:?}", first.as_rule()),
        }
    }

    fn parse_component_stmt(&mut self, pair: Pair<Rule>) -> Result<Statement> {
        ensure!(pair.as_rule() == Rule::component_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();

        let name = self.parse_ident(name)?;
        let fields = self.parse_typed_idents(inner)?;

        Ok(Statement::Component(Component { name, fields }))
    }

    fn parse_system_stmt(&mut self, pair: Pair<Rule>) -> Result<Statement> {
        ensure!(pair.as_rule() == Rule::system_stmt);
        let mut inner = pair.into_inner();
        let tag = inner.next().unwrap();
        let (name, tag) = if tag.as_rule() == Rule::system_tag {
            (inner.next().unwrap(), Some(tag.as_str().to_string()))
        } else {
            (tag, None)
        };

        let name = self.parse_ident(name)?;

        let block = inner.next().unwrap();

        let block = self.parse_block(block)?;
        let block = if let Expr::Block(block) = block.expr {
            block
        } else {
            bail!("Expected block statement");
        };
        Ok(Statement::System(System {
            tag,
            name: Box::new(name),
            block,
        }))
    }

    fn parse_func_stmt(&mut self, pair: Pair<Rule>) -> Result<Statement> {
        ensure!(pair.as_rule() == Rule::func_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();
        let params = inner.next().unwrap();
        let ret_type = inner.next().unwrap();
        let (block, ret_type) = if ret_type.as_rule() == Rule::r#type {
            (inner.next().unwrap(), Some(self.parse_type(ret_type)?))
        } else {
            (ret_type, None)
        };

        let name = self.parse_ident(name)?;
        let params = self.parse_typed_args(params.into_inner())?;
        let block = self.parse_block(block)?;
        let block = if let Expr::Block(block) = block.expr {
            block
        } else {
            bail!("Expected block statement");
        };

        Ok(Statement::Func(Func {
            name: Box::new(name),
            params,
            ret_type: ret_type.map(Box::new),
            block,
        }))
    }

    fn parse_impl_stmt(&mut self, pair: Pair<Rule>) -> Result<Statement> {
        ensure!(pair.as_rule() == Rule::impl_stmt);
        let mut inner = pair.into_inner();
        let ty = inner.next().unwrap();
        let block = inner.next().unwrap();

        let ty = self.parse_type(ty)?;
        let block = self.parse_block(block)?;
        let block = if let Expr::Block(block) = block.expr {
            block
        } else {
            bail!("Expected block statement");
        };

        let mut funcs = Vec::new();
        for stmt in &block.statements {
            if let Statement::Func(func) = stmt {
                funcs.push(func.clone());
            }
        }

        Ok(Statement::Impl(Impl {
            ty: Box::new(ty),
            funcs,
        }))
    }

    fn parse_query_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::query_expr);
        let mut inner = pair.clone().into_inner();
        let name = inner.next().unwrap();
        let components = inner.next().unwrap();
        let rest = inner;

        let name = self.parse_ident(name)?;
        let components = self.parse_typed_decls(components)?;
        let (with, without) = self.parse_with_without(rest)?;

        Ok(SpanExpr::new(
            pair,
            Expr::Query(Query {
                name: Box::new(name),
                components,
                with,
                without,
            }),
        ))
    }

    fn parse_typed_decls(&mut self, pair: Pair<Rule>) -> Result<Vec<TypedIdent>> {
        ensure!(pair.as_rule() == Rule::typed_decls);
        let mut fields = Vec::new();
        for field in pair.into_inner() {
            match field.as_rule() {
                Rule::var_typed_ident => fields.push(self.parse_var_typed_ident(field)?),
                Rule::let_typed_ident => fields.push(self.parse_typed_ident(field)?),
                _ => bail!("Unexpected rule: {:?}", field.as_rule()),
            }
        }
        Ok(fields)
    }

    fn parse_with_without(&mut self, pair: Pairs<Rule>) -> Result<(Vec<SpanExpr>, Vec<SpanExpr>)> {
        let mut with = Vec::new();
        let mut without = Vec::new();
        for pair in pair {
            match pair.as_rule() {
                Rule::with_clause => {
                    let mut inner = pair.into_inner();
                    let name = inner.next().unwrap();
                    let name = self.parse_type(name)?;
                    with.push(name);
                }
                Rule::without_clause => {
                    let mut inner = pair.into_inner();
                    let name = inner.next().unwrap();
                    let name = self.parse_type(name)?;
                    without.push(name);
                }
                _ => bail!("Unexpected rule: {:?}", pair.as_rule()),
            }
        }
        Ok((with, without))
    }

    fn parse_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        let inner = pair.into_inner();

        let pratt = PrattParser::new()
            .op(Op::infix(Rule::plus, Assoc::Left) | Op::infix(Rule::minus, Assoc::Left))
            .op(Op::infix(Rule::star, Assoc::Left) | Op::infix(Rule::slash, Assoc::Left))
            .op(Op::prefix(Rule::minus))
            .op(Op::infix(Rule::lt, Assoc::Left)
                | Op::infix(Rule::gt, Assoc::Left)
                | Op::infix(Rule::lte, Assoc::Left)
                | Op::infix(Rule::gte, Assoc::Left)
                | Op::infix(Rule::eqeq, Assoc::Left)
                | Op::infix(Rule::neq, Assoc::Left))
            .op(Op::prefix(Rule::not))
            .op(Op::infix(Rule::and, Assoc::Left))
            .op(Op::infix(Rule::or, Assoc::Left))
            .op(Op::infix(Rule::xor, Assoc::Left))
            .op(Op::infix(Rule::eq, Assoc::Right));

        pratt
            .map_primary(|primary| {
                Ok(match primary.as_rule() {
                    Rule::ident => self.parse_ident(primary.clone())?,
                    Rule::int_literal => SpanExpr::new(
                        primary.clone(),
                        Expr::IntLiteral(primary.as_str().parse().unwrap()),
                    ),
                    Rule::float_literal => SpanExpr::new(
                        primary.clone(),
                        Expr::FloatLiteral(primary.as_str().parse().unwrap()),
                    ),
                    Rule::string_literal => {
                        let string = primary.as_str();
                        SpanExpr::new(
                            primary.clone(),
                            Expr::StringLiteral(string[1..string.len() - 1].to_string()),
                        )
                    }
                    Rule::call_expr => self.parse_call_expr(primary)?,
                    Rule::block => self.parse_block(primary)?,
                    Rule::statements => {
                        let statements = self.parse_statements(primary.clone())?;
                        SpanExpr::new(primary, Expr::Block(Block { statements }))
                    }
                    Rule::expr => self.parse_expr(primary)?, // parenthesized expression
                    Rule::assign_expr => self.parse_assign_expr(primary)?,
                    Rule::member_expr => self.parse_member_expr(primary)?,
                    Rule::decl_expr => self.parse_decl_expr(primary)?,
                    Rule::constructor_expr => self.parse_constructor_expr(primary)?,
                    Rule::if_expr => self.parse_if_expr(primary)?,
                    Rule::loop_expr => self.parse_loop_expr(primary)?,
                    Rule::query_expr => self.parse_query_expr(primary)?,
                    Rule::var_res => self.parse_var_res(primary)?,
                    Rule::let_res => self.parse_let_res(primary)?,
                    _ => bail!("Unexpected rule: {:?}", primary.as_rule()),
                })
            })
            .map_prefix(|op, rhs| {
                let op_str = match op.as_rule() {
                    Rule::plus => "+".to_string(),
                    Rule::minus => "-".to_string(),
                    _ => bail!("Unexpected rule: {:?}", op.as_rule()),
                };

                Ok(SpanExpr::new(
                    op,
                    Expr::Prefix {
                        op: op_str,
                        rhs: Box::new(rhs?),
                    },
                ))
            })
            .map_infix(|lhs, op, rhs| {
                let op_str = match op.as_rule() {
                    Rule::plus => "+".to_string(),
                    Rule::minus => "-".to_string(),
                    Rule::star => "*".to_string(),
                    Rule::slash => "/".to_string(),
                    Rule::lt => "<".to_string(),
                    Rule::gt => ">".to_string(),
                    Rule::lte => "<=".to_string(),
                    Rule::gte => ">=".to_string(),
                    Rule::eqeq => "==".to_string(),
                    Rule::neq => "!=".to_string(),
                    Rule::and => "&&".to_string(),
                    Rule::or => "||".to_string(),
                    Rule::xor => "^".to_string(),
                    _ => bail!("Unexpected rule: {:?}", op.as_rule()),
                };

                Ok(SpanExpr::new(
                    op,
                    Expr::Infix {
                        op: op_str,
                        lhs: Box::new(lhs?),
                        rhs: Box::new(rhs?),
                    },
                ))
            })
            .parse(inner)
    }

    fn parse_assign_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::assign_expr);
        let mut inner = pair.clone().into_inner();
        let lhs = inner.next().unwrap();
        let op = inner.next().unwrap();
        let rhs = inner.next().unwrap();

        let lhs = match lhs.as_rule() {
            Rule::ident => self.parse_ident(lhs)?,
            Rule::member_expr => self.parse_member_expr(lhs)?,
            _ => bail!("Unexpected rule: {:?}", lhs.as_rule()),
        };

        let rhs = self.parse_expr(rhs)?;

        Ok(SpanExpr::new(
            pair,
            Expr::Infix {
                op: op.as_str().to_string(),
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        ))
    }

    fn parse_member_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::member_expr);
        let mut inner = pair.clone().into_inner();
        let lhs = inner.next().unwrap();
        let rhs = inner.next().unwrap();

        let lhs = match lhs.as_rule() {
            Rule::ident => self.parse_ident(lhs)?,
            Rule::member_expr => self.parse_member_expr(lhs)?,
            Rule::r#type => self.parse_type(lhs)?,
            _ => bail!("Unexpected rule: {:?}", lhs.as_rule()),
        };

        let rhs = match rhs.as_rule() {
            Rule::ident => self.parse_ident(rhs)?,
            Rule::call_expr => self.parse_call_expr(rhs)?,
            _ => bail!("Unexpected rule: {:?}", rhs.as_rule()),
        };

        Ok(SpanExpr::new(
            pair,
            Expr::Member {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        ))
    }

    fn parse_decl_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::decl_expr);
        let mut inner = pair.clone().into_inner();
        let mutability = inner.next().unwrap();
        let _eq = inner.next().unwrap();

        match mutability.as_rule() {
            Rule::var_typed_ident => {
                let ident = self.parse_var_typed_ident(mutability.into_inner().next().unwrap())?;
                let initial_value = inner.next().unwrap();
                let initial_value = self.parse_expr(initial_value)?;
                Ok(SpanExpr::new(
                    pair,
                    Expr::Decl {
                        mutability: true,
                        ident: Box::new(ident.name),
                        initial_value: Box::new(initial_value),
                    },
                ))
            }
            Rule::let_typed_ident => {
                let ident = self.parse_typed_ident(mutability.into_inner().next().unwrap())?;
                let initial_value = inner.next().unwrap();
                let initial_value = self.parse_expr(initial_value)?;
                Ok(SpanExpr::new(
                    pair,
                    Expr::Decl {
                        mutability: false,
                        ident: Box::new(ident.name),
                        initial_value: Box::new(initial_value),
                    },
                ))
            }
            Rule::var_ident => {
                let ident = self.parse_ident(mutability.into_inner().next().unwrap())?;
                let initial_value = inner.next().unwrap();
                let initial_value = self.parse_expr(initial_value)?;
                Ok(SpanExpr::new(
                    pair,
                    Expr::Decl {
                        mutability: true,
                        ident: Box::new(ident),
                        initial_value: Box::new(initial_value),
                    },
                ))
            }
            Rule::let_ident => {
                let ident = self.parse_ident(mutability.into_inner().next().unwrap())?;
                let initial_value = inner.next().unwrap();
                let initial_value = self.parse_expr(initial_value)?;
                Ok(SpanExpr::new(
                    pair,
                    Expr::Decl {
                        mutability: false,
                        ident: Box::new(ident),
                        initial_value: Box::new(initial_value),
                    },
                ))
            }
            _ => bail!("Unexpected rule: {:?}", mutability.as_rule()),
        }
    }

    fn parse_constructor_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::constructor_expr);
        let mut inner = pair.clone().into_inner();
        let name = inner.next().unwrap();

        let name = self.parse_type(name)?;

        let mut args_vec = Vec::new();

        while let Some(arg) = inner.next() {
            match arg.as_rule() {
                Rule::ident => {
                    let ident = arg.as_str().to_string();
                    let expr = inner.next().unwrap();
                    let expr = self.parse_expr(expr)?;
                    args_vec.push((ident, expr));
                }
                _ => bail!("Unexpected rule: {:?} {}", arg.as_rule(), arg.as_str()),
            }
        }

        Ok(SpanExpr::new(
            pair,
            Expr::Construct {
                name: Box::new(name),
                args: args_vec,
            },
        ))
    }

    fn parse_if_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::if_expr);
        let mut inner = pair.clone().into_inner();
        let condition = inner.next().unwrap();
        let then_block = inner.next().unwrap();

        let mut elif_blocks = Vec::new();
        let mut else_block = None;

        while let Some(block) = inner.next() {
            match block.as_rule() {
                Rule::elif => {
                    let condition = inner.next().unwrap();
                    let block = inner.next().unwrap();

                    let condition = self.parse_expr(condition)?;
                    let block = self.parse_expr(block)?;

                    elif_blocks.push((Box::new(condition), Box::new(block)));
                }
                Rule::r#else => {
                    let block = inner.next().unwrap();
                    let block = self.parse_expr(block)?;
                    else_block = Some(Box::new(block));
                }
                _ => bail!("Unexpected rule: {:?}", block.as_rule()),
            }
        }

        let condition = self.parse_expr(condition)?;
        let then_block = self.parse_expr(then_block)?;

        Ok(SpanExpr::new(
            pair,
            Expr::If {
                condition: Box::new(condition),
                then_block: Box::new(then_block),
                elif_blocks,
                else_block,
            },
        ))
    }

    fn parse_loop_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::loop_expr);
        let mut inner = pair.clone().into_inner();
        let condition = inner.next().unwrap();

        let (condition, block) = if condition.as_rule() == Rule::expr {
            let block = inner.next().unwrap();
            let condition = self.parse_expr(condition)?;
            let block = self.parse_expr(block)?;
            (Some(Box::new(condition)), Box::new(block))
        } else {
            let block = self.parse_expr(condition)?;
            (None, Box::new(block))
        };

        Ok(SpanExpr::new(pair, Expr::Loop { condition, block }))
    }

    fn parse_block(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::block);
        let mut inner = pair.clone().into_inner();
        let stmts = inner.next().unwrap();

        let statements = self.parse_statements(stmts)?;

        Ok(SpanExpr::new(pair, Expr::Block(Block { statements })))
    }

    fn parse_call_expr(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::call_expr);
        let mut inner = pair.clone().into_inner();
        let name = inner.next().unwrap();

        let name = self.parse_ident(name)?;

        let mut args_vec = Vec::new();

        for arg in inner {
            match arg.as_rule() {
                Rule::expr => args_vec.push(self.parse_expr(arg)?),
                Rule::ident => args_vec.push(self.parse_ident(arg)?),
                _ => bail!("Unexpected rule: {:?} {}", arg.as_rule(), arg.as_str()),
            }
        }

        Ok(SpanExpr::new(
            pair,
            Expr::Call(Call {
                name: Box::new(name),
                args: args_vec,
            }),
        ))
    }

    fn parse_ident(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(
            matches!(pair.as_rule(), Rule::ident | Rule::capitalized_ident,),
            "{:?}",
            pair.as_rule()
        );
        Ok(SpanExpr::new(
            pair.clone(),
            Expr::Ident(pair.as_str().to_string()),
        ))
    }

    fn parse_type(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::r#type);
        Ok(SpanExpr::new(
            pair.clone(),
            Expr::Type(pair.as_str().to_string()),
        ))
    }

    fn parse_var_res(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::var_res);
        let mut inner = pair.clone().into_inner();
        let ident = inner.next().unwrap();
        let _eq = inner.next().unwrap();
        let res = inner.next().unwrap();

        let ident = self.parse_ident(ident)?;
        let res = self.parse_type(res)?;

        let expr = Expr::Res {
            mutability: true,
            ident: Box::new(ident),
            res: Box::new(res),
        };
        Ok(SpanExpr::new(pair, expr))
    }

    fn parse_let_res(&mut self, pair: Pair<Rule>) -> Result<SpanExpr> {
        ensure!(pair.as_rule() == Rule::let_res);
        let mut inner = pair.clone().into_inner();
        let ident = inner.next().unwrap();
        let _eq = inner.next().unwrap();
        let res = inner.next().unwrap();

        let ident = self.parse_ident(ident)?;
        let res = self.parse_type(res)?;

        Ok(SpanExpr::new(
            pair,
            Expr::Res {
                mutability: false,
                ident: Box::new(ident),
                res: Box::new(res),
            },
        ))
    }

    fn parse_var_typed_ident(&mut self, pair: Pair<Rule>) -> Result<TypedIdent> {
        ensure!(pair.as_rule() == Rule::var_typed_ident);
        let mut inner = pair.into_inner();
        let ident = inner.next().unwrap();
        let ty = inner.next().unwrap();

        let name = self.parse_ident(ident)?;
        let ty = self.parse_type(ty)?;

        Ok(TypedIdent {
            mutability: true,
            name,
            ty,
        })
    }

    fn parse_typed_ident(&mut self, pair: Pair<Rule>) -> Result<TypedIdent> {
        ensure!(matches!(
            pair.as_rule(),
            Rule::typed_ident | Rule::let_typed_ident
        ));
        let mut inner = pair.into_inner();
        let ident = inner.next().unwrap();
        let ty = inner.next().unwrap();

        let name = self.parse_ident(ident)?;
        let ty = self.parse_type(ty)?;

        Ok(TypedIdent {
            mutability: false,
            name,
            ty,
        })
    }

    fn parse_typed_idents(&mut self, pair: Pairs<Rule>) -> Result<Vec<TypedIdent>> {
        let mut fields = Vec::new();
        for field in pair {
            match field.as_rule() {
                Rule::typed_ident => fields.push(self.parse_typed_ident(field)?),
                Rule::var_typed_ident => fields.push(self.parse_var_typed_ident(field)?),
                Rule::let_typed_ident => fields.push(self.parse_typed_ident(field)?),
                _ => bail!("Unexpected rule: {:?}", field.as_rule()),
            }
        }
        Ok(fields)
    }

    fn parse_typed_args(&mut self, pair: Pairs<Rule>) -> Result<Vec<TypedIdent>> {
        let mut args = Vec::new();
        for arg in self.parse_typed_idents(pair)?.into_iter() {
            args.push(arg);
        }
        Ok(args)
    }
}

impl Default for LoomParser {
    fn default() -> Self {
        Self::new()
    }
}
