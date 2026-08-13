use anyhow::{bail};
use crate::scanner::{Scanner, Token, TokenType};

pub enum Statement {
    Query(Query),
    // Alter,
    // Insert,
    // Update,
    // Delete,
    // Drop
}

pub struct Query{
    select: Select,
    order: Vec<OrderBy>,
    limit: Option<Limit>
}

pub struct Limit {
    count: usize,
    offset: Option<usize>
}

pub struct OrderBy {
    expr: Expr,
    asc: bool
}


pub struct Select {
    projection: Vec<Expr>,
    from: TableExpr,
    filter: Option<Expr>
}

pub enum TableExpr {
    Table(String),


    Join {
        left: Box<TableExpr>,
        kind: JoinType,
        right: Box<TableExpr>,
        cond: Expr
    }
}

pub enum JoinType {
    Inner,
    Left,
    Right,
    Full
}

pub enum Expr {
    Column(String),
    UnaryOp(UnaryOp),
    BinaryOp(BinaryOp),
    Literal(Literal)
}

pub struct UnaryOp {
    op: UnaryOperation,
    operand: Box<Expr>
}

pub struct BinaryOp {
    left: Box<Expr>,
    op: BinaryOperation,
    right: Box<Expr>
}

pub enum BinaryOperation {
    Add,
    Divide,
    Multiply,
    Subtract,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    And,
    Or,
}

pub enum UnaryOperation {
    Not,
    Plus,
    Minus
}

pub enum Literal {
    Boolean(bool),
    Integer(isize),
    String(String),
    Null
} 
 
struct Parser<'a, 'src> {
    scanner: &'a mut Scanner<'src>,
    current: Token,
}

impl <'a, 'src> Parser<'a, 'src> {
    fn init(scanner: &'a mut Scanner<'src>) -> anyhow::Result<Self> {
        let current = scanner.scan_token()?;
        
        Ok(Parser {
            scanner,
            current,
        })
    }

    fn advance(&mut self) -> anyhow::Result<()>{
        self.current = self.scanner.scan_token()?;
        Ok(())
    } 

    fn parse(&mut self) -> anyhow::Result<Option<Statement>>{
        if Scanner::is_eof(&self.current) {
            return Ok(None)
        }

        let statement = match self.current.t_type {
            TokenType::Select => Statement::Query( Query{
                select: self.parse_select()?,
                order: vec!(),
                limit: None, 
            }),
            _ => bail!("Unexpected token {:?}", self.current.t_type),
        };

        Ok(Some(statement))
    }


    fn parse_select(&mut self) -> anyhow::Result<Select> {
        Ok(Select { projection: self.parse_projection()?, from: self.parse_from()?, filter: self.parse_filter()? })
    }

    fn parse_projection(&mut self) -> anyhow::Result<Vec<Expr>> {
        let mut projections = vec!();
        
        self.advance()?;



        while matches!(self.current.t_type, TokenType::From) {
            
            
            // TokenType::Comma => self.advance(),
            // TokenType::Identifier => 

            self.advance();
        }


        Ok(projections)
    }

    fn parse_from(&mut self) -> anyhow::Result<TableExpr> {
        Ok(TableExpr::Table(String::from("hello")))
    }

    fn parse_filter(&mut self) -> anyhow::Result<Option<Expr>> {
        Ok(None)
    }

}


pub fn interpret(source: &str) -> anyhow::Result<()> {
    let mut scanner = Scanner::init_scanner(source);
    let mut parser = Parser::init(&mut scanner)?;

    let _ = parser.parse()?;

    Ok(())
}
