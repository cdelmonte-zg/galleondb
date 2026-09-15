use std::io::ErrorKind::TimedOut;

use crate::{
    scanner::{Scanner, Token, TokenType},
    semantic::LogicalType,
};
use anyhow::bail;

#[derive(Debug)]
pub(crate) enum Statement {
    Query(Query),
    CreateTable(CreateTable),
    // Alter,
    // Insert,
    // Update,
    // Delete,
    // Drop,
}

#[derive(Debug)]
pub(crate) struct CreateTable {
    pub(crate) name: String,
    pub(crate) columns: Vec<ColumnDefinition>,
}

#[derive(Debug)]
pub(crate) struct ColumnDefinition {
    pub(crate) name: String,
    pub(crate) declared_type: DeclaredType,
    pub(crate) constraints: Vec<ColumnConstraint>,
}

#[derive(Debug)]
pub(crate) struct DeclaredType {
    pub(crate) name: String,
    pub(crate) parameters: Vec<u32>,
}

#[derive(Debug)]
pub(crate) enum ColumnConstraint {
    NotNull,
    Null,
    PrimaryKey,
    Autoincrement
}

#[derive(Debug)]
pub(crate) struct Query {
    pub(crate) select: Select,
    pub(crate) order: Vec<OrderBy>,
    pub(crate) limit: Option<Limit>,
}

#[derive(Debug)]
pub(crate) struct Limit {
    count: Expr,
    offset: Option<Expr>,
}

#[derive(Debug)]
pub(crate) struct OrderBy {
    expr: Expr,
    asc: bool,
}

#[derive(Debug)]
pub(crate) struct Select {
    pub(crate) projection: Vec<SelectionItem>,
    pub(crate) from: Option<TableExpr>,
    pub(crate) filter: Option<Expr>,
}

#[derive(Debug)]
pub(crate) enum SelectionItem {
    SelectionExpr(Expr),
    Wildcard,
}

#[derive(Debug)]
pub enum TableExpr {
    Table(String),
    Join {
        left: Box<TableExpr>,
        kind: JoinType,
        right: Box<TableExpr>,
        cond: Expr,
    },
}

#[derive(Debug)]
pub(crate) enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug)]
pub(crate) enum Expr {
    Column(String),
    UnaryOp(UnaryOp),
    BinaryOp(BinaryOp),
    Literal(Literal),
    FunctionCall(FunctionCall),
}

#[derive(Debug)]
pub(crate) struct UnaryOp {
    op: UnaryOperation,
    operand: Box<Expr>,
}

#[derive(Debug)]
pub(crate) struct BinaryOp {
    left: Box<Expr>,
    op: BinaryOperation,
    right: Box<Expr>,
}

#[derive(Debug)]
pub(crate) struct FunctionCall {
    pub(crate) name: String,
    pub(crate) args: Vec<FunctionArgument>,
}

#[derive(Debug)]
pub(crate) enum FunctionArgument {
    Expr(Expr),
    Wildcard,
}

#[derive(Debug)]
pub(crate) enum BinaryOperation {
    Add,
    Divide,
    Multiply,
    Subtract,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterEqualThan,
    LessThan,
    LessEqualThan,
    And,
    Or,
}

#[derive(Debug)]
pub(crate) enum UnaryOperation {
    Not,
    Plus,
    Minus,
}

#[derive(Debug)]
pub(crate) enum Literal {
    Boolean(bool),
    Integer(isize),
    Float(f64),
    String(String),
    Null,
}

#[derive(Debug)]
struct Parser<'a, 'src> {
    scanner: &'a mut Scanner<'src>,
    current: Token,
    previous: Token,
}

impl<'a, 'src> Parser<'a, 'src> {
    fn init(scanner: &'a mut Scanner<'src>) -> anyhow::Result<Self> {
        let current = scanner.scan_token()?;

        Ok(Parser {
            scanner,
            current,
            previous: Token {
                t_type: TokenType::Start,
                start: 0,
                length: 0,
                line: 1,
            },
        })
    }

    fn advance(&mut self) -> anyhow::Result<()> {
        self.previous = self.current.clone();
        self.current = self.scanner.scan_token()?;

        println!("{:?}", self.current);

        Ok(())
    }

    fn expect(&mut self, expected_t_type: TokenType) -> anyhow::Result<()> {
        if self.current.t_type != expected_t_type {
            bail!(
                "Current Token Type {:?} is not the expected one {:?}.",
                self.current.t_type,
                expected_t_type
            );
        }

        self.advance()?;
        Ok(())
    }

    fn parse(&mut self) -> anyhow::Result<Option<Statement>> {
        if Scanner::is_eof(&self.current) {
            return Ok(None);
        }

        let statement = match self.current.t_type {
            TokenType::Select => {
                self.advance()?;

                let select = self.parse_select()?;
                let order = self.parse_order_by()?;
                let limit = self.parse_limit()?;

                Statement::Query(Query {
                    select,
                    order,
                    limit,
                })
            }

            TokenType::Create => {
                self.advance()?;
                self.expect(TokenType::Table)?;

                if self.current.t_type != TokenType::Identifier {
                    bail!("Expected table name.");
                }

                let name = self.current_value()?.to_string();
                self.advance()?;

                let columns = self.parse_column_definitions()?;

                Statement::CreateTable(CreateTable { name, columns })
            }

            _ => bail!("Unexpected token {:?}", self.current.t_type),
        };

        if self.current.t_type == TokenType::Semicolon {
            self.advance()?;
        }

        if !Scanner::is_eof(&self.current) {
            bail!("Unexpected token {:?}", self.current.t_type);
        }

        Ok(Some(statement))
    }

    fn parse_column_definitions(&mut self) -> anyhow::Result<Vec<ColumnDefinition>> {
        self.expect(TokenType::LeftParen)?;

        let mut column_definitions = vec![];
        column_definitions.push(self.parse_column_definition()?);

        while self.current.t_type == TokenType::Comma {
            self.advance()?;

            column_definitions.push(self.parse_column_definition()?);
        }

        self.expect(TokenType::RightParen)?;

        Ok(column_definitions)
    }

    fn parse_column_definition(&mut self) -> anyhow::Result<ColumnDefinition> {
        if self.current.t_type != TokenType::Identifier {
            bail!("Expected column name.");
        }
        let name = self.current_value()?.to_string();
        self.advance()?;

        let declared_type = self.parse_declared_type()?;
        let constraints = self.parse_column_constraints()?;

        let column_definition = ColumnDefinition {
            name,
            declared_type,
            constraints,
        };

        Ok(column_definition)
    }

    fn parse_declared_type(&mut self) -> anyhow::Result<DeclaredType> {
        if self.current.t_type != TokenType::Identifier {
            bail!("Expected declared type.");
        }

        let name = self.current_value()?.to_ascii_uppercase();
        self.advance()?;

        let parameters = if self.current.t_type == TokenType::LeftParen {
            self.parse_type_parameters()?
        } else {
            vec![]
        };

        Ok(DeclaredType { name, parameters })
    }

    fn parse_type_parameters(&mut self) -> anyhow::Result<Vec<u32>> {
        let mut parameters = vec![];

        self.expect(TokenType::LeftParen)?;

        if self.current.t_type != TokenType::Number {
            bail!("Number expected as parameter");
        }

        parameters.push(self.current_value()?.parse::<u32>()?);
        self.advance()?;

        while self.current.t_type == TokenType::Comma {
            self.advance()?;

            if self.current.t_type != TokenType::Number {
                bail!("Number expected as parameter");
            }
            parameters.push(self.current_value()?.parse::<u32>()?);

            self.advance()?;
        }

        self.expect(TokenType::RightParen)?;

        Ok(parameters)
    }

    fn parse_column_constraints(&mut self) -> anyhow::Result<Vec<ColumnConstraint>> {
        let mut constraints = vec![];

        loop {
            match self.current.t_type {
                TokenType::Not => {
                    self.advance()?;
                    self.expect(TokenType::Null)?;

                    constraints.push(ColumnConstraint::NotNull);
                }
                
                TokenType::Null => {
                    self.advance()?;

                    constraints.push(ColumnConstraint::Null);
                }

                TokenType::Primary => {
                    self.advance()?;
                    self.expect(TokenType::Key)?;

                    constraints.push(ColumnConstraint::PrimaryKey);
                }

                TokenType::Autoincrement => {
                    self.advance()?;
                    constraints.push(ColumnConstraint::Autoincrement);
                }

                TokenType::Comma | TokenType::RightParen => {
                    break;
                }
                
                _ => bail!(
                    "Unespected token in column constrain {:?}",
                    self.current.t_type
                ),
            }
        }

        Ok(constraints)
    }

    fn parse_select(&mut self) -> anyhow::Result<Select> {
        Ok(Select {
            projection: self.parse_projection()?,
            from: self.parse_from()?,
            filter: self.parse_filter()?,
        })
    }

    fn parse_order_by(&mut self) -> anyhow::Result<Vec<OrderBy>> {
        if self.current.t_type != TokenType::Order {
            return Ok(vec![]);
        }
        self.advance()?;

        self.expect(TokenType::By)?;

        let mut expressions: Vec<OrderBy> = vec![];

        loop {
            let expr = self.parse_expression()?;

            let asc = match self.current.t_type {
                TokenType::Asc => {
                    self.advance()?;
                    true
                }
                TokenType::Desc => {
                    self.advance()?;
                    false
                }
                _ => true,
            };

            expressions.push(OrderBy { expr, asc });

            if self.current.t_type == TokenType::Comma {
                self.advance()?;
                continue;
            }
            break;
        }

        Ok(expressions)
    }

    fn parse_limit(&mut self) -> anyhow::Result<Option<Limit>> {
        if self.current.t_type != TokenType::Limit {
            return Ok(None);
        }

        self.advance()?;

        let count = self.parse_expression()?;

        let offset = if self.current.t_type == TokenType::Offset {
            self.advance()?;
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Some(Limit { count, offset }))
    }

    fn parse_from(&mut self) -> anyhow::Result<Option<TableExpr>> {
        if self.current.t_type != TokenType::From {
            return Ok(None);
        }
        self.advance()?;

        if self.current.t_type != TokenType::Identifier {
            bail!("Expected table name.");
        }

        let mut table_expr = TableExpr::Table(self.current_value()?.to_string());

        self.advance()?;

        while matches!(
            self.current.t_type,
            TokenType::Join
                | TokenType::Left
                | TokenType::Right
                | TokenType::Inner
                | TokenType::Full
        ) {
            table_expr = self.parse_join(table_expr)?;
        }

        Ok(Some(table_expr))
    }

    fn parse_join(&mut self, table_expr: TableExpr) -> anyhow::Result<TableExpr> {
        let kind = match self.current.t_type {
            TokenType::Join => {
                self.advance()?;
                JoinType::Inner
            }
            TokenType::Left => {
                self.advance()?;
                self.expect(TokenType::Join)?;
                JoinType::Left
            }
            TokenType::Right => {
                self.advance()?;
                self.expect(TokenType::Join)?;
                JoinType::Right
            }
            TokenType::Full => {
                self.advance()?;
                self.expect(TokenType::Join)?;
                JoinType::Full
            }
            TokenType::Inner => {
                self.advance()?;
                self.expect(TokenType::Join)?;
                JoinType::Inner
            }
            _ => {
                bail!("Join type expected, found {:?}", self.current.t_type);
            }
        };

        if self.current.t_type != TokenType::Identifier {
            bail!("Table name expected, found {:?}", self.current.t_type);
        }

        let right_table_expr = TableExpr::Table(self.current_value()?.to_string());

        self.advance()?;

        self.expect(TokenType::On)?;

        let cond = self.parse_expression()?;

        Ok(TableExpr::Join {
            left: Box::new(table_expr),
            kind,
            right: Box::new(right_table_expr),
            cond,
        })
    }

    fn parse_expression(&mut self) -> anyhow::Result<Expr> {
        self.parse_or()
    }

    // or → and ("OR" and)*
    fn parse_or(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_and()?;

        while self.current.t_type == TokenType::Or {
            self.advance()?;

            let right = self.parse_and()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op: BinaryOperation::Or,
                right: Box::new(right),
            });
        }

        Ok(expr)
    }

    // and → not ("AND" not)*
    fn parse_and(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_not()?;

        while self.current.t_type == TokenType::And {
            self.advance()?;

            let right = self.parse_not()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op: BinaryOperation::And,
                right: Box::new(right),
            });
        }

        Ok(expr)
    }

    // not → "NOT" not | equality
    fn parse_not(&mut self) -> anyhow::Result<Expr> {
        if self.current.t_type == TokenType::Not {
            self.advance()?;

            let operand = self.parse_not()?;

            return Ok(Expr::UnaryOp(UnaryOp {
                op: UnaryOperation::Not,
                operand: Box::new(operand),
            }));
        }

        self.parse_equality()
    }

    // equality → comparison (("=" | "!=") comparison)*
    fn parse_equality(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_comparison()?;

        while self.current.t_type == TokenType::Equal || self.current.t_type == TokenType::BangEqual
        {
            let op = if self.current.t_type == TokenType::Equal {
                BinaryOperation::Equal
            } else {
                BinaryOperation::NotEqual
            };

            self.advance()?;

            let right = self.parse_comparison()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            });
        }

        Ok(expr)
    }

    // comparison  → term (("<" | ">" | "<=" | ">=") term)*
    fn parse_comparison(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_term()?;

        while matches!(
            self.current.t_type,
            TokenType::Less | TokenType::Greater | TokenType::LessEqual | TokenType::GreaterEqual
        ) {
            let op = match self.current.t_type {
                TokenType::Less => BinaryOperation::LessThan,
                TokenType::LessEqual => BinaryOperation::LessEqualThan,
                TokenType::Greater => BinaryOperation::GreaterThan,
                TokenType::GreaterEqual => BinaryOperation::GreaterEqualThan,
                _ => bail!("Wrong comparison operator: {:?} ", self.current.t_type),
            };

            self.advance()?;

            let right = self.parse_term()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            })
        }

        Ok(expr)
    }

    // term → factor (("+" | "-") factor)*
    fn parse_term(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_factor()?;

        while matches!(self.current.t_type, TokenType::Plus | TokenType::Minus) {
            let op = if self.current.t_type == TokenType::Plus {
                BinaryOperation::Add
            } else {
                BinaryOperation::Subtract
            };

            self.advance()?;

            let right = self.parse_factor()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            });
        }

        Ok(expr)
    }

    // factor → unary (("*" | "/") unary)*
    fn parse_factor(&mut self) -> anyhow::Result<Expr> {
        let mut expr = self.parse_unary()?;

        while matches!(self.current.t_type, TokenType::Star | TokenType::Slash) {
            let op = if self.current.t_type == TokenType::Star {
                BinaryOperation::Multiply
            } else {
                BinaryOperation::Divide
            };

            self.advance()?;

            let right = self.parse_unary()?;

            expr = Expr::BinaryOp(BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            });
        }

        Ok(expr)
    }

    // unary       → ("-" | "+") unary
    //             | primary
    fn parse_unary(&mut self) -> anyhow::Result<Expr> {
        if matches!(self.current.t_type, TokenType::Minus | TokenType::Plus) {
            let op = match self.current.t_type {
                TokenType::Minus => UnaryOperation::Minus,
                TokenType::Plus => UnaryOperation::Plus,
                _ => unreachable!(),
            };

            self.advance()?;
            let operand = Box::new(self.parse_unary()?);

            return Ok(Expr::UnaryOp(UnaryOp { op, operand }));
        }

        self.parse_primary()
    }

    // primary     → NUMBER
    //             | STRING
    //             | TRUE
    //             | FALSE
    //             | NULL
    //             | IDENTIFIER
    //             | IDENTIFIER "(" arguments? ")"
    //             | "(" expression ")"
    fn parse_primary(&mut self) -> anyhow::Result<Expr> {
        let expr = match self.current.t_type {
            TokenType::Number => {
                let value = self.current_value()?;

                if value.contains('.') {
                    Expr::Literal(Literal::Float(value.parse::<f64>()?))
                } else {
                    Expr::Literal(Literal::Integer(value.parse::<isize>()?))
                }
            }
            TokenType::Identifier => {
                let name = self.current_value()?.to_string();
                self.advance()?;

                if self.current.t_type == TokenType::LeftParen {
                    self.advance()?;

                    let args = if self.current.t_type == TokenType::RightParen {
                        vec![]
                    } else {
                        self.parse_arguments()?
                    };

                    self.expect(TokenType::RightParen)?;

                    return Ok(Expr::FunctionCall(FunctionCall { name, args }));
                } else {
                    return Ok(Expr::Column(name));
                }
            }
            TokenType::String => Expr::Literal(Literal::String(self.current_string_value()?)),
            TokenType::True => Expr::Literal(Literal::Boolean(true)),
            TokenType::False => Expr::Literal(Literal::Boolean(false)),
            TokenType::Null => Expr::Literal(Literal::Null),
            TokenType::LeftParen => {
                self.advance()?;
                let expr = self.parse_expression()?;
                self.expect(TokenType::RightParen)?;
                return Ok(expr);
            }
            _ => bail!("Expected expression: {:?}", self.current.t_type),
        };

        self.advance()?;
        Ok(expr)
    }

    // arguments -> argument ("," argument)*
    fn parse_arguments(&mut self) -> anyhow::Result<Vec<FunctionArgument>> {
        let mut arguments = vec![];

        loop {
            arguments.push(self.parse_function_argument()?);

            if self.current.t_type == TokenType::Comma {
                self.advance()?;
                continue;
            }

            break;
        }

        Ok(arguments)
    }

    fn parse_function_argument(&mut self) -> anyhow::Result<FunctionArgument> {
        if self.current.t_type == TokenType::Star {
            self.advance()?;
            Ok(FunctionArgument::Wildcard)
        } else {
            Ok(FunctionArgument::Expr(self.parse_expression()?))
        }
    }

    fn current_string_value(&self) -> anyhow::Result<String> {
        let value = self.current_value()?;
        let inner = &value[1..value.len() - 1];

        Ok(inner.replace("''", "'"))
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

    fn parse_projection(&mut self) -> anyhow::Result<Vec<SelectionItem>> {
        let mut projections = vec![];

        loop {
            if self.current.t_type == TokenType::Star {
                self.advance()?;
                projections.push(SelectionItem::Wildcard)
            } else {
                projections.push(SelectionItem::SelectionExpr(self.parse_expression()?))
            }

            if self.current.t_type == TokenType::Comma {
                self.advance()?;
                continue;
            }

            break;
        }

        Ok(projections)
    }

    fn parse_filter(&mut self) -> anyhow::Result<Option<Expr>> {
        if self.current.t_type != TokenType::Where {
            return Ok(None);
        }

        self.advance()?;
        Ok(Some(self.parse_expression()?))
    }
}

pub fn parse(source: &str) -> anyhow::Result<Option<Statement>> {
    let mut scanner = Scanner::init_scanner(source);
    let mut parser = Parser::init(&mut scanner)?;

    let statement = parser.parse()?;

    Ok(statement)
}
