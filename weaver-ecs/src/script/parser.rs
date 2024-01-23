use pest::{
    iterators::{Pair, Pairs},
    pratt_parser::{Assoc, Op, PrattParser},
    Parser,
};
use pest_derive::Parser;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedIdent {
    pub mutability: bool,
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub fields: Vec<TypedIdent>,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    Call(Call),
    Block(Block),
    Member {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Infix {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Prefix {
        op: String,
        rhs: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum Statement {
    Component(Component),
    System(System),
    Query(Query),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub scoped_idents: Vec<TypedIdent>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub name: String,
    pub components: Vec<TypedIdent>,
    pub with: Vec<String>,
    pub without: Vec<String>,
    pub iter_type: String,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct System {
    pub name: String,
    pub queries: Vec<Query>,
    pub block: Block,
}

#[derive(Debug)]
pub enum Scope {
    Program(Vec<Scope>),
    Component(Component),
    System(System),
}

impl Default for Scope {
    fn default() -> Self {
        Scope::Program(Vec::new())
    }
}

#[derive(Parser)]
#[grammar = "../weaver-ecs/src/script/loom.pest"]
pub struct LoomParser {
    query_counter: usize,
    top_scope: Scope,
}

impl LoomParser {
    pub fn new() -> Self {
        Self {
            query_counter: 0,
            top_scope: Scope::default(),
        }
    }

    pub fn finish(self) -> Vec<Scope> {
        match self.top_scope {
            Scope::Program(scopes) => scopes,
            _ => panic!("Unexpected scope"),
        }
    }

    pub fn parse_script(&mut self, script: &str) -> anyhow::Result<()> {
        let mut pairs = LoomParser::parse(Rule::program, script)?;

        let program = pairs.next().unwrap();
        assert_eq!(program.as_rule(), Rule::program);

        let start = program.into_inner().next().unwrap();
        assert_eq!(start.as_rule(), Rule::statements);
        for pair in start.into_inner() {
            match pair.as_rule() {
                Rule::statements => {
                    let stmts = self.parse_statements(pair);
                    for stmt in stmts {
                        self.push_statement(stmt);
                    }
                }
                Rule::statement => {
                    let stmt = self.parse_statement(pair);
                    self.push_statement(stmt);
                }
                _ => panic!("Unexpected rule: {:?}", pair.as_rule()),
            }
        }

        Ok(())
    }

    fn push_statement(&mut self, stmt: Statement) {
        match &mut self.top_scope {
            Scope::Program(stmts) => match stmt {
                Statement::Component(component) => {
                    stmts.push(Scope::Component(component));
                }
                Statement::System(system) => {
                    stmts.push(Scope::System(system));
                }
                _ => panic!("Unexpected statement"),
            },
            _ => panic!("Unexpected scope"),
        }
    }

    fn parse_statements(&mut self, pair: Pair<Rule>) -> Vec<Statement> {
        assert_eq!(pair.as_rule(), Rule::statements);
        let mut statements = Vec::new();
        for stmt in pair.into_inner() {
            let stmt = match stmt.as_rule() {
                Rule::statement => self.parse_statement(stmt),
                _ => panic!("Unexpected rule: {:?}", stmt.as_rule()),
            };
            statements.push(stmt);
        }
        statements
    }

    fn parse_statement(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::statement);
        let mut inner = pair.into_inner();
        let first = inner.next().unwrap();

        match first.as_rule() {
            Rule::component_stmt => self.parse_component_stmt(first),
            Rule::system_stmt => self.parse_system_stmt(first),
            Rule::query_stmt => self.parse_query_stmt(first),
            Rule::expr => Statement::Expr(self.parse_expr(first)),
            _ => panic!("Unexpected rule: {:?}", first.as_rule()),
        }
    }

    fn parse_component_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::component_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();

        let name = self.parse_ident(name);
        let fields = self.parse_typed_idents(inner);

        Statement::Component(Component { name, fields })
    }

    #[allow(clippy::only_used_in_recursion)]
    fn extract_queries(&mut self, block: &Block) -> Vec<Query> {
        let mut queries = Vec::new();
        for stmt in &block.statements {
            match stmt {
                Statement::Query(query) => {
                    queries.push(query.clone());
                    queries.extend(self.extract_queries(&query.block));
                }
                Statement::Expr(Expr::Block(block)) => {
                    queries.extend(self.extract_queries(block));
                }
                _ => {}
            }
        }

        queries
    }

    fn parse_system_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::system_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();

        let block = inner.next().unwrap();

        let block = self.parse_block(block);
        let block = if let Expr::Block(block) = block {
            block
        } else {
            panic!("Expected block statement");
        };
        let queries = self.extract_queries(&block);
        Statement::System(System {
            name: name.as_str().to_string(),
            queries,
            block,
        })
    }

    fn parse_query_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::query_stmt);
        let mut inner = pair.into_inner();
        let components = inner.next().unwrap();

        let block;
        let mut with_clause = None;
        let mut without_clause = None;

        let next = inner.next().unwrap();
        match next.as_rule() {
            Rule::block => {
                block = next;
            }
            Rule::with_clause => {
                with_clause = Some(next);

                let next = inner.next().unwrap();
                match next.as_rule() {
                    Rule::block => {
                        block = next;
                    }
                    Rule::without_clause => {
                        without_clause = Some(next);
                        block = inner.next().unwrap();
                    }
                    _ => panic!("Unexpected rule: {:?}", next.as_rule()),
                }
            }
            Rule::without_clause => {
                without_clause = Some(next);

                let next = inner.next().unwrap();

                match next.as_rule() {
                    Rule::block => {
                        block = next;
                    }
                    _ => panic!("Unexpected rule: {:?}", next.as_rule()),
                }
            }
            _ => panic!("Unexpected rule: {:?}", next.as_rule()),
        }

        let name = format!("query_{}", self.query_counter);
        self.query_counter += 1;
        let components = self.parse_typed_args(components.into_inner());
        let block = self.parse_block(block);
        let block = if let Expr::Block(block) = block {
            block
        } else {
            panic!("Expected block statement");
        };

        let mut with = Vec::new();
        let mut without = Vec::new();

        if let Some(with_clause) = with_clause {
            let mut inner = with_clause.into_inner();
            let with_pair = inner.next().unwrap();
            with_pair.into_inner().for_each(|ident| {
                with.push(self.parse_ident(ident));
            });
        }

        if let Some(without_clause) = without_clause {
            let mut inner = without_clause.into_inner();
            let without_pair = inner.next().unwrap();
            without_pair.into_inner().for_each(|ident| {
                without.push(self.parse_ident(ident));
            });
        }

        Statement::Query(Query {
            name,
            components,
            with,
            without,
            iter_type: "foreach".to_string(),
            block,
        })
    }

    fn extract_scoped_idents(&mut self, block: Pair<Rule>) -> Vec<TypedIdent> {
        let mut scoped_idents = Vec::new();
        for stmt in block.into_inner() {
            let stmts = match stmt.as_rule() {
                Rule::statements => self.parse_statements(stmt),
                Rule::statement => vec![self.parse_statement(stmt)],
                _ => panic!("Unexpected rule: {:?}", stmt.as_rule()),
            };
            for stmt in stmts {
                if let Statement::Query(query) = stmt {
                    scoped_idents.extend(query.components);
                }
            }
        }
        scoped_idents
    }

    fn parse_expr(&mut self, pair: Pair<Rule>) -> Expr {
        let inner = pair.into_inner();

        let pratt = PrattParser::new()
            .op(Op::infix(Rule::plus, Assoc::Left) | Op::infix(Rule::minus, Assoc::Left))
            .op(Op::infix(Rule::star, Assoc::Left) | Op::infix(Rule::slash, Assoc::Left))
            .op(Op::infix(Rule::eq, Assoc::Right));

        pratt
            .map_primary(|primary| match primary.as_rule() {
                Rule::ident => Expr::Ident(self.parse_ident(primary)),
                Rule::int_literal => Expr::IntLiteral(primary.as_str().parse().unwrap()),
                Rule::float_literal => Expr::FloatLiteral(primary.as_str().parse().unwrap()),
                Rule::string_literal => Expr::StringLiteral(primary.as_str().to_string()),
                Rule::call_expr => self.parse_call_expr(primary),
                Rule::block => self.parse_block(primary),
                Rule::expr => self.parse_expr(primary), // parenthesized expression
                Rule::assign_expr => self.parse_assign_expr(primary),
                Rule::member_expr => self.parse_member_expr(primary),
                _ => panic!("Unexpected rule: {:?}", primary.as_rule()),
            })
            .map_prefix(|op, rhs| {
                let op = match op.as_rule() {
                    Rule::plus => "+".to_string(),
                    Rule::minus => "-".to_string(),
                    _ => panic!("Unexpected rule: {:?}", op.as_rule()),
                };

                Expr::Prefix {
                    op,
                    rhs: Box::new(rhs),
                }
            })
            .map_infix(|lhs, op, rhs| {
                let op = match op.as_rule() {
                    Rule::plus => "+".to_string(),
                    Rule::minus => "-".to_string(),
                    Rule::star => "*".to_string(),
                    Rule::slash => "/".to_string(),
                    _ => panic!("Unexpected rule: {:?}", op.as_rule()),
                };

                Expr::Infix {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            })
            .parse(inner)
    }

    fn parse_assign_expr(&mut self, pair: Pair<Rule>) -> Expr {
        assert_eq!(pair.as_rule(), Rule::assign_expr);
        let mut inner = pair.into_inner();
        let lhs = inner.next().unwrap();
        let op = inner.next().unwrap();
        let rhs = inner.next().unwrap();

        let lhs = match lhs.as_rule() {
            Rule::ident => Expr::Ident(self.parse_ident(lhs)),
            Rule::member_expr => self.parse_member_expr(lhs),
            _ => panic!("Unexpected rule: {:?}", lhs.as_rule()),
        };

        let rhs = self.parse_expr(rhs);

        Expr::Infix {
            op: op.as_str().to_string(),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn parse_member_expr(&mut self, pair: Pair<Rule>) -> Expr {
        assert_eq!(pair.as_rule(), Rule::member_expr);
        let mut inner = pair.into_inner();
        let lhs = inner.next().unwrap();
        let rhs = inner.next().unwrap();

        let lhs = match lhs.as_rule() {
            Rule::ident => Expr::Ident(self.parse_ident(lhs)),
            Rule::member_expr => self.parse_member_expr(lhs),
            _ => panic!("Unexpected rule: {:?}", lhs.as_rule()),
        };

        let rhs = match rhs.as_rule() {
            Rule::ident => Expr::Ident(self.parse_ident(rhs)),
            _ => panic!("Unexpected rule: {:?}", rhs.as_rule()),
        };

        Expr::Member {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn parse_block(&mut self, pair: Pair<Rule>) -> Expr {
        assert_eq!(pair.as_rule(), Rule::block);
        let mut inner = pair.into_inner();
        let stmts = inner.next().unwrap();

        let scoped_idents = self.extract_scoped_idents(stmts.clone());
        let stmts = self.parse_statements(stmts);

        Expr::Block(Block {
            statements: stmts,
            scoped_idents,
        })
    }

    fn parse_call_expr(&mut self, pair: Pair<Rule>) -> Expr {
        assert_eq!(pair.as_rule(), Rule::call_expr);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();

        let name = self.parse_ident(name);

        let mut args_vec = Vec::new();

        for arg in inner {
            match arg.as_rule() {
                Rule::expr => args_vec.push(self.parse_expr(arg)),
                Rule::ident => args_vec.push(Expr::Ident(self.parse_ident(arg))),
                _ => panic!("Unexpected rule: {:?} {}", arg.as_rule(), arg.as_str()),
            }
        }

        Expr::Call(Call {
            name,
            args: args_vec,
        })
    }

    fn parse_ident(&mut self, pair: Pair<Rule>) -> String {
        assert!(matches!(pair.as_rule(), Rule::ident));
        pair.as_str().to_string()
    }

    fn parse_type(&mut self, pair: Pair<Rule>) -> String {
        assert_eq!(pair.as_rule(), Rule::typ);
        pair.as_str().to_string()
    }

    fn parse_mut_typed_ident(&mut self, pair: Pair<Rule>) -> TypedIdent {
        assert_eq!(pair.as_rule(), Rule::mut_typed_ident);
        let mut inner = pair.into_inner();
        let ident = inner.next().unwrap();
        let ty = inner.next().unwrap();

        let ident = self.parse_ident(ident);
        let ty = self.parse_type(ty);

        TypedIdent {
            mutability: true,
            name: ident.as_str().to_string(),
            ty: ty.as_str().to_string(),
        }
    }

    fn parse_typed_ident(&mut self, pair: Pair<Rule>) -> TypedIdent {
        assert_eq!(pair.as_rule(), Rule::typed_ident);
        let mut inner = pair.into_inner();
        let ident = inner.next().unwrap();
        let ty = inner.next().unwrap();

        let ident = self.parse_ident(ident);
        let ty = self.parse_type(ty);

        TypedIdent {
            mutability: false,
            name: ident.as_str().to_string(),
            ty: ty.as_str().to_string(),
        }
    }

    fn parse_typed_idents(&mut self, pair: Pairs<Rule>) -> Vec<TypedIdent> {
        let mut fields = Vec::new();
        for field in pair {
            match field.as_rule() {
                Rule::typed_ident => fields.push(self.parse_typed_ident(field)),
                Rule::mut_typed_ident => fields.push(self.parse_mut_typed_ident(field)),
                _ => panic!("Unexpected rule: {:?}", field.as_rule()),
            }
        }
        fields
    }

    fn parse_typed_args(&mut self, pair: Pairs<Rule>) -> Vec<TypedIdent> {
        let mut args = Vec::new();
        for arg in self.parse_typed_idents(pair).into_iter() {
            args.push(arg);
        }
        args
    }
}

impl Default for LoomParser {
    fn default() -> Self {
        Self::new()
    }
}