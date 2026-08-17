use anyhow::bail;
use crate::{parser::TableExpr::JoinExpr, scanner::{Scanner, Token, TokenType::{self, Join}}};

#[derive(Debug)]
pub enum Statement {
    Query(Query),
    // Alter,
    // Insert,
    // Update,
    // Delete,
    // Drop
}

#[derive(Debug)]
pub struct Query{
    select: Select,
    order: Vec<OrderBy>,
    limit: Option<Limit>
}

#[derive(Debug)]
pub struct Limit {
    count: usize,
    offset: Option<usize>
}

#[derive(Debug)]
pub struct OrderBy {
    expr: Expr,
    asc: bool
}

#[derive(Debug)]
pub struct Select {
    projection: Vec<Expr>,
    from: TableExpr,
    filter: Option<Expr>
}

#[derive(Debug)]
pub enum TableExpr {
    Table(String),
    JoinExpr {
        left: Box<TableExpr>,
        kind: JoinType,
        right: Box<TableExpr>,
        cond: Expr
    }
}

#[derive(Debug)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full
}

#[derive(Debug)]
pub enum Expr {
    Column(String),
    UnaryOp(UnaryOp),
    BinaryOp(BinaryOp),
    Literal(Literal)
}

#[derive(Debug)]
pub struct UnaryOp {
    op: UnaryOperation,
    operand: Box<Expr>
}

#[derive(Debug)]
pub struct BinaryOp {
    left: Box<Expr>,
    op: BinaryOperation,
    right: Box<Expr>
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum UnaryOperation {
    Not,
    Plus,
    Minus
}

#[derive(Debug)]
pub enum Literal {
    Boolean(bool),
    Integer(isize),
    String(String),
    Null
} 

#[derive(Debug)]
struct Parser<'a, 'src> {
    scanner: &'a mut Scanner<'src>,
    current: Token,
    previous: Token,
}

impl <'a, 'src> Parser<'a, 'src> {
    fn init(scanner: &'a mut Scanner<'src>) -> anyhow::Result<Self> {
        let current = scanner.scan_token()?;
        
        Ok(Parser {
            scanner,
            current,
            previous : Token {
                t_type: TokenType::Start,
                start: 0,
                length: 0,
                line: 1
            }
        })
    }

    fn advance(&mut self) -> anyhow::Result<()>{
        self.previous = self.current.clone();
        self.current = self.scanner.scan_token()?;
        println!("{:?}", self.current);
        Ok(())
    }

    fn expect(&mut self, t_type: TokenType) -> anyhow::Result<bool> {
        self.advance()?;
        if self.current.t_type == t_type {
            Ok(true) 
        } else {
            Ok(false)
        }
    }

    fn parse(&mut self) -> anyhow::Result<Option<Statement>>{
        if Scanner::is_eof(&self.current) {
            return Ok(None)
        }

        let statement = match self.current.t_type {
            TokenType::Select => {           
                self.advance()?;

                Statement::Query( Query{
                    select: self.parse_select()?,
                    order: vec!(),
                    limit: None, 
                })
            },
            _ => bail!("Unexpected token {:?}", self.current.t_type),
        };

        println!("{:?}", statement);

        Ok(Some(statement))
    }

    fn parse_select(&mut self) -> anyhow::Result<Select> {
        Ok(Select { projection: self.parse_projection()?, from: self.parse_from()?, filter: self.parse_filter()? })
    }

    fn parse_from(&mut self) -> anyhow::Result<TableExpr> {
        self.advance()?;
        let table_expr = TableExpr::Table(self.current_value()?.to_string());
        
        self.advance()?;
        match self.current.t_type {
            TokenType::Where | TokenType::Eof => Ok(table_expr),
            _ => self.parse_join(table_expr),
        }
    }

    fn parse_join(&mut self, table_expr: TableExpr) -> anyhow::Result<TableExpr> {
        if Scanner::is_eof(&self.current) {
            return Ok(table_expr)
        }

        let kind = if self.current.t_type == TokenType::Join {
                JoinType::Inner
        } else {
                match self.current.t_type {
                TokenType::Left => {
                    if !self.expect(TokenType::Join)? {
                        bail!("Invalid syntax")
                    };
                    JoinType::Left
                },
                TokenType::Right  => {
                    if !self.expect(TokenType::Join)? {
                        bail!("Invalid syntax")
                    };
                    JoinType::Right
                },
                TokenType::Inner => {
                    if !self.expect(TokenType::Join)? {
                        bail!("Invalid syntax")
                    };
                    JoinType::Inner
                },
                TokenType::Full => {
                    if !self.expect(TokenType::Join)? {
                        bail!("Invalid syntax")
                    };
                    JoinType::Full
                },
                _ => bail!("Malformed From clause: {:?}", self.current.t_type)
            }
        };

        if !self.expect(TokenType::Identifier)? {
            bail!("identifier expected, found {:?}", self.current.t_type);
        }
        
        let right = Box::new(TableExpr::Table(self.current_value()?.to_string()));        
        
        if !self.expect(TokenType::On)? {
            bail!("expected token condition, found {:?}", self.current.t_type)
        }

        let cond = Expr::BinaryOp(self.parse_binary_op()?);

        let join_expr = TableExpr::JoinExpr { 
            left: Box::new(table_expr),
            kind,
            right,
            cond
        };

        self.advance()?;

        if self.current.t_type == TokenType::Where || Scanner::is_eof(&self.current){
            return Ok(join_expr)
        } else {
            return Ok(self.parse_join(join_expr)?)
        }
    }


    fn parse_binary_op(&mut self) -> anyhow::Result<BinaryOp>{
        
        let left= ;
        let op,
        let right,
        
        BinaryOp {
            left,
            op,
            right,
        }
    }

    fn current_value(&self) -> Result<&str, std::str::Utf8Error> {
        let source = self.scanner.source;

        let value = &source[self.current.start..(self.current.start + self.current.length)];

        str::from_utf8(value)
    }

    fn previous_value(&self) -> Result<&str, std::str::Utf8Error> {
        let source = self.scanner.source;

        let value = &source[self.previous.start..(self.previous.start + self.previous.length)];

        str::from_utf8(value)
    }

    fn parse_projection(&mut self) -> anyhow::Result<Vec<Expr>> {
        let mut projections = vec!();

        while !matches!(self.current.t_type, TokenType::From) {   
            if Scanner::is_eof(&self.current) {
                bail!("Select needs a From");
            }
            
            projections.push(match self.current.t_type {
                TokenType::Number => Expr::Literal(self::Literal::Integer(self.current_value()?.parse::<isize>()?)),
                TokenType::Identifier => Expr::Column(self.current_value()?.to_string()),
                TokenType::True => Expr::Literal(Literal::Boolean(true)),
                TokenType::False => Expr::Literal(Literal::Boolean(false)),
                TokenType::Null => Expr::Literal(Literal::Null),
                TokenType::Comma => {
                    self.advance()?;
                    continue;
                }
                _ => bail!("Projection token not recognized"),
            });
            self.advance()?;
        }

        Ok(projections)
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
