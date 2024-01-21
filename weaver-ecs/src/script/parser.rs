use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Debug)]
pub struct TypedIdent {
    pub name: String,
    pub ty: String,
}

#[derive(Debug)]
pub struct Component {
    pub name: String,
    pub fields: Vec<TypedIdent>,
}

#[derive(Debug)]
pub struct Call {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub enum Statement {
    Component(Component),
    System(System),
    Query(Query),
    Call(Call),
    Block(Block),
}

#[derive(Debug)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Debug)]
pub struct Query {
    pub components: Vec<TypedIdent>,
    pub iter_type: String,
    pub block: Block,
}

#[derive(Debug)]
pub struct System {
    pub name: String,
    pub queries: Vec<Query>,
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

#[derive(Parser, Default, Debug)]
#[grammar = "../weaver-ecs/src/script/loom.pest"]
pub struct LoomParser {
    top_scope: Scope,
}

impl LoomParser {
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
            Rule::call_stmt => self.parse_call_stmt(first),
            _ => panic!("Unexpected rule: {:?}", first.as_rule()),
        }
    }

    fn parse_component_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::component_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();
        let fields = inner.next().unwrap();

        let name = self.parse_ident(name);
        let fields = self.parse_typed_idents(fields);

        Statement::Component(Component { name, fields })
    }

    fn extract_queries(&mut self, block: Pair<Rule>) -> Vec<Query> {
        let mut queries = Vec::new();
        for stmt in block.into_inner() {
            let stmts = match stmt.as_rule() {
                Rule::statements => self.parse_statements(stmt),
                _ => panic!("Unexpected rule: {:?}", stmt.as_rule()),
            };
            for stmt in stmts {
                if let Statement::Query(query) = stmt {
                    queries.push(query);
                } else {
                    panic!("Expected query statement");
                }
            }
        }
        queries
    }

    fn parse_system_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::system_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();
        let stmts = inner.next().unwrap();

        let queries = self.extract_queries(stmts);
        Statement::System(System {
            name: name.as_str().to_string(),
            queries,
        })
    }

    fn parse_query_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::query_stmt);
        let mut inner = pair.into_inner();
        let components = inner.next().unwrap();
        let block = inner.next().unwrap();

        let components = self.parse_typed_args(components);
        let block = self.parse_block_stmt(block);
        let block = if let Statement::Block(block) = block {
            block
        } else {
            panic!("Expected block statement");
        };

        Statement::Query(Query {
            components,
            iter_type: "foreach".to_string(),
            block,
        })
    }

    fn parse_block_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::block_stmt);
        let mut inner = pair.into_inner();
        let stmts = inner.next().unwrap();

        let stmts = self.parse_statements(stmts);

        Statement::Block(Block { statements: stmts })
    }

    fn parse_call_stmt(&mut self, pair: Pair<Rule>) -> Statement {
        assert_eq!(pair.as_rule(), Rule::call_stmt);
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap();
        let args = inner.next().unwrap();

        let name = self.parse_ident(name);
        let args = self.parse_args(args);

        Statement::Call(Call { name, args })
    }

    fn parse_ident(&mut self, pair: Pair<Rule>) -> String {
        assert_eq!(pair.as_rule(), Rule::ident);
        pair.as_str().to_string()
    }

    fn parse_type(&mut self, pair: Pair<Rule>) -> String {
        assert_eq!(pair.as_rule(), Rule::r#type);
        pair.as_str().to_string()
    }

    fn parse_typed_ident(&mut self, pair: Pair<Rule>) -> TypedIdent {
        assert_eq!(pair.as_rule(), Rule::typed_ident);
        let mut inner = pair.into_inner();
        let ident = inner.next().unwrap();
        let ty = inner.next().unwrap();

        let ident = self.parse_ident(ident);
        let ty = self.parse_type(ty);

        TypedIdent {
            name: ident.as_str().to_string(),
            ty: ty.as_str().to_string(),
        }
    }

    fn parse_typed_idents(&mut self, pair: Pair<Rule>) -> Vec<TypedIdent> {
        let mut fields = Vec::new();
        for field in pair.into_inner() {
            fields.push(self.parse_typed_ident(field));
        }
        fields
    }

    fn parse_args(&mut self, pair: Pair<Rule>) -> Vec<String> {
        assert_eq!(pair.as_rule(), Rule::args);
        let mut args = Vec::new();
        for arg in pair.into_inner() {
            args.push(self.parse_ident(arg));
        }
        args
    }

    fn parse_typed_args(&mut self, pair: Pair<Rule>) -> Vec<TypedIdent> {
        assert_eq!(pair.as_rule(), Rule::typed_args);
        let mut args = Vec::new();
        for arg in pair.into_inner() {
            args.push(self.parse_typed_ident(arg));
        }
        args
    }
}
