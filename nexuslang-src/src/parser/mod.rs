use crate::ast::*;
use crate::diagnostic::Diagnostic;
/// NexusLang Parser — transforma tokens em AST
use crate::lexer::Token;

type ParseResult<T> = Result<T, Diagnostic>;

pub struct Parser {
    tokens: Vec<(Token, usize, usize)>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, usize)>) -> Self {
        Parser {
            tokens: tokens
                .into_iter()
                .map(|(token, line)| (token, line, 0))
                .collect(),
            pos: 0,
        }
    }

    pub fn new_spanned(tokens: Vec<(Token, usize, usize)>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|(t, _, _)| t)
            .unwrap_or(&Token::Eof)
    }

    fn peek2(&self) -> &Token {
        self.tokens
            .get(self.pos + 1)
            .map(|(t, _, _)| t)
            .unwrap_or(&Token::Eof)
    }

    fn peek3(&self) -> &Token {
        self.tokens
            .get(self.pos + 2)
            .map(|(t, _, _)| t)
            .unwrap_or(&Token::Eof)
    }

    fn current_line(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, l, _)| *l).unwrap_or(0)
    }

    fn current_column(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, _, c)| *c).unwrap_or(0)
    }

    fn current_span(&self) -> Span {
        Span::new(self.current_line(), self.current_column())
    }

    fn error_at(&self, line: usize, column: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic::parser(message, line, column)
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        self.error_at(self.current_line(), self.current_column(), message)
    }

    fn advance(&mut self) -> Token {
        let tok = self
            .tokens
            .get(self.pos)
            .map(|(t, _, _)| t.clone())
            .unwrap_or(Token::Eof);
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> ParseResult<Token> {
        let line = self.current_line();
        let column = self.current_column();
        let tok = self.advance();
        if std::mem::discriminant(&tok) == std::mem::discriminant(expected) {
            Ok(tok)
        } else {
            Err(self.error_at(
                line,
                column,
                format!("esperado {:?}, encontrado {:?}", expected, tok),
            ))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        let line = self.current_line();
        let column = self.current_column();
        match self.advance() {
            Token::Ident(name) => Ok(name),
            tok => Err(self.error_at(
                line,
                column,
                format!("esperado identificador, encontrado {:?}", tok),
            )),
        }
    }

    fn at(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(expected)
    }

    fn consume_comma_or_end(
        &mut self,
        end: &Token,
        end_label: &str,
        item_label: &str,
    ) -> ParseResult<bool> {
        if self.at(end) {
            return Ok(false);
        }

        let line = self.current_line();
        let column = self.current_column();
        if self.at(&Token::Comma) {
            self.advance();
            if self.at(end) {
                return Err(self.error_at(
                    line,
                    column,
                    format!("{} esperado apos ','", item_label),
                ));
            }
            return Ok(true);
        }

        Err(self.error_at(
            line,
            column,
            format!(
                "esperado ',' ou '{}', encontrado {:?}",
                end_label,
                self.peek()
            ),
        ))
    }

    fn parse_expr_list(
        &mut self,
        end: Token,
        end_label: &str,
        item_label: &str,
    ) -> ParseResult<Vec<Expr>> {
        let mut items = Vec::new();
        while !self.at(&end) && !self.at(&Token::Eof) {
            items.push(self.parse_expr()?);
            if !self.consume_comma_or_end(&end, end_label, item_label)? {
                break;
            }
        }
        Ok(items)
    }

    fn parse_object_fields(&mut self) -> ParseResult<Vec<ObjectField>> {
        let mut fields = Vec::new();
        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            let span = self.current_span();
            let name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let value = self.parse_expr()?;
            fields.push(ObjectField { name, value, span });

            if !self.consume_comma_or_end(&Token::RBrace, "}", "campo")? {
                break;
            }
        }
        Ok(fields)
    }

    fn parse_type(&mut self) -> ParseResult<Type> {
        let ty = self.parse_type_base()?;
        if *self.peek() == Token::Question {
            self.advance();
            Ok(Type::Optional(Box::new(ty)))
        } else {
            Ok(ty)
        }
    }

    fn parse_type_base(&mut self) -> ParseResult<Type> {
        match self.peek().clone() {
            Token::TypeString => {
                self.advance();
                Ok(Type::String)
            }
            Token::TypeInt => {
                self.advance();
                Ok(Type::Int)
            }
            Token::TypeFloat => {
                self.advance();
                Ok(Type::Float)
            }
            Token::TypeBool => {
                self.advance();
                Ok(Type::Bool)
            }
            Token::TypeMoney => {
                self.advance();
                Ok(Type::Money)
            }
            Token::TypeDate => {
                self.advance();
                Ok(Type::Date)
            }
            Token::Ident(name) => {
                self.advance();
                Ok(Type::Model(name))
            }
            Token::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(&Token::RBracket)?;
                Ok(Type::Array(Box::new(inner)))
            }
            tok => Err(self.error(format!("tipo esperado, encontrado {:?}", tok))),
        }
    }

    /// Parse the entire program
    pub fn parse_program(&mut self) -> Result<Program, String> {
        self.parse_program_diagnostic()
            .map_err(|diagnostic| diagnostic.to_string())
    }

    pub fn parse_program_diagnostic(&mut self) -> ParseResult<Program> {
        let mut decls = Vec::new();

        while *self.peek() != Token::Eof {
            let decl = self.parse_decl()?;
            decls.push(decl);
        }

        Ok(Program { decls })
    }

    fn parse_decl(&mut self) -> ParseResult<Decl> {
        match self.peek().clone() {
            Token::Fn => self.parse_function(),
            Token::Model => self.parse_model(),
            Token::Workflow => self.parse_workflow(),
            Token::Route => self.parse_route(),
            Token::Invoice => self.parse_invoice(),
            _ => {
                let stmt = self.parse_stmt()?;
                Ok(Decl::Statement(stmt))
            }
        }
    }

    fn parse_function(&mut self) -> ParseResult<Decl> {
        let span = self.current_span();
        self.advance(); // consume 'fn'
        let name = self.expect_ident()?;

        self.expect(&Token::LParen)?;
        let mut params = Vec::new();

        while *self.peek() != Token::RParen && *self.peek() != Token::Eof {
            let param_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let param_type = self.parse_type()?;
            params.push((param_name, param_type));

            if !self.consume_comma_or_end(&Token::RParen, ")", "parametro")? {
                break;
            }
        }
        self.expect(&Token::RParen)?;

        let return_type = if *self.peek() == Token::Arrow {
            self.advance();
            self.parse_type()?
        } else {
            Type::Void
        };

        let body = self.parse_block()?;

        Ok(Decl::Function {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_model(&mut self) -> ParseResult<Decl> {
        let span = self.current_span();
        self.advance(); // consume 'model'
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;

        let mut fields = Vec::new();
        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            let field_span = self.current_span();
            let field_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let field_type = self.parse_type()?;
            let mut unique = false;
            let mut index = false;
            let mut min = None;
            let mut max = None;
            self.parse_model_field_constraints(
                &field_name,
                &mut unique,
                &mut index,
                &mut min,
                &mut max,
            )?;
            let default = if self.at(&Token::Assign) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.parse_model_field_constraints(
                &field_name,
                &mut unique,
                &mut index,
                &mut min,
                &mut max,
            )?;
            fields.push(Field {
                name: field_name,
                ty: field_type,
                default,
                unique,
                index,
                min,
                max,
                span: field_span,
            });
        }

        self.expect(&Token::RBrace)?;
        Ok(Decl::Model { name, fields, span })
    }

    fn parse_model_field_constraints(
        &mut self,
        field_name: &str,
        unique: &mut bool,
        index: &mut bool,
        min: &mut Option<Expr>,
        max: &mut Option<Expr>,
    ) -> ParseResult<()> {
        loop {
            let Token::Ident(name) = self.peek().clone() else {
                return Ok(());
            };
            if matches!(self.peek2(), Token::Colon) {
                return Ok(());
            }

            let span = self.current_span();
            match name.as_str() {
                "unique" => {
                    if *unique {
                        return Err(self.error_at(
                            span.line,
                            span.column,
                            format!("constraint 'unique' duplicada no campo '{}'", field_name),
                        ));
                    }

                    self.advance();
                    *unique = true;
                }
                "index" => {
                    if *index {
                        return Err(self.error_at(
                            span.line,
                            span.column,
                            format!("constraint 'index' duplicada no campo '{}'", field_name),
                        ));
                    }

                    self.advance();
                    *index = true;
                }
                "min" => {
                    if min.is_some() {
                        return Err(self.error_at(
                            span.line,
                            span.column,
                            format!("constraint 'min' duplicada no campo '{}'", field_name),
                        ));
                    }

                    self.advance();
                    *min = Some(self.parse_expr()?);
                }
                "max" => {
                    if max.is_some() {
                        return Err(self.error_at(
                            span.line,
                            span.column,
                            format!("constraint 'max' duplicada no campo '{}'", field_name),
                        ));
                    }

                    self.advance();
                    *max = Some(self.parse_expr()?);
                }
                _ => {
                    return Err(self.error_at(
                        span.line,
                        span.column,
                        format!(
                            "constraint de campo '{}' desconhecida: '{}'",
                            field_name, name
                        ),
                    ));
                }
            }
        }
    }

    fn parse_workflow(&mut self) -> ParseResult<Decl> {
        let span = self.current_span();
        self.advance(); // consume 'workflow'
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;

        let mut steps = Vec::new();
        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            if *self.peek() == Token::Step {
                let step_span = self.current_span();
                self.advance();
                let step_name = self.expect_ident()?;
                let body = if *self.peek() == Token::LBrace {
                    self.parse_block()?
                } else {
                    Vec::new()
                };
                steps.push(WorkflowStep {
                    name: step_name,
                    body,
                    span: step_span,
                });
            } else {
                return Err(self.error(format!(
                    "esperado 'step' em workflow, encontrado {:?}",
                    self.peek()
                )));
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(Decl::Workflow { name, steps, span })
    }

    fn parse_route(&mut self) -> ParseResult<Decl> {
        let span = self.current_span();
        self.advance(); // consume 'route'

        let method_line = self.current_line();
        let method_column = self.current_column();
        let method = match self.advance() {
            Token::Get => HttpMethod::Get,
            Token::Post => HttpMethod::Post,
            Token::Put => HttpMethod::Put,
            Token::Delete => HttpMethod::Delete,
            tok => {
                return Err(self.error_at(
                    method_line,
                    method_column,
                    format!("método HTTP inválido: {:?}", tok),
                ))
            }
        };

        // Parse path: /employees, /employees/search-page or /employees/:id
        if !self.at(&Token::Slash) {
            return Err(self.error("route path deve comecar com '/'"));
        }

        let mut path = String::new();
        let mut params = Vec::new();
        while *self.peek() == Token::Slash
            || *self.peek() == Token::Colon
            || *self.peek() == Token::Minus
            || *self.peek() == Token::In
            || matches!(self.peek(), Token::Ident(_))
        {
            match self.advance() {
                Token::Slash => path.push('/'),
                Token::Minus => path.push('-'),
                Token::In => path.push_str("in"),
                Token::Ident(s) => path.push_str(&s),
                Token::Colon => {
                    path.push(':');
                    let param = self.expect_ident()?;
                    path.push_str(&param);
                    params.push(param);
                }
                _ => {}
            }
        }

        let query_params = if *self.peek() == Token::Question {
            self.parse_route_query_params()?
        } else {
            Vec::new()
        };

        let body = self.parse_block()?;
        Ok(Decl::Route {
            method,
            path,
            params,
            query_params,
            body,
            span,
        })
    }

    fn parse_route_query_params(&mut self) -> ParseResult<Vec<QueryParam>> {
        self.expect(&Token::Question)?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        while !self.at(&Token::RParen) && !self.at(&Token::Eof) {
            let span = self.current_span();
            let name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.at(&Token::Assign) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(QueryParam {
                name,
                ty,
                default,
                span,
            });

            if !self.consume_comma_or_end(&Token::RParen, ")", "query param")? {
                break;
            }
        }

        self.expect(&Token::RParen)?;
        Ok(params)
    }

    fn parse_invoice(&mut self) -> ParseResult<Decl> {
        let span = self.current_span();
        self.advance(); // consume 'invoice'
        self.expect(&Token::LBrace)?;

        let mut fields = Vec::new();
        let mut items = Vec::new();
        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            if matches!(self.peek(), Token::Ident(s) if s == "item") {
                let item_span = self.current_span();
                self.advance();
                let description = self.parse_expr()?;
                let qty_line = self.current_line();
                let qty_column = self.current_column();
                match self.advance() {
                    Token::Ident(s) if s == "qty" => {}
                    tok => {
                        return Err(self.error_at(
                            qty_line,
                            qty_column,
                            format!("esperado 'qty', encontrado {:?}", tok),
                        ))
                    }
                }
                let qty = self.parse_expr()?;
                let price_line = self.current_line();
                let price_column = self.current_column();
                match self.advance() {
                    Token::Ident(s) if s == "price" => {}
                    tok => {
                        return Err(self.error_at(
                            price_line,
                            price_column,
                            format!("esperado 'price', encontrado {:?}", tok),
                        ))
                    }
                }
                let price = self.parse_expr()?;
                items.push(InvoiceItem {
                    description,
                    qty,
                    price,
                    span: item_span,
                });

                if *self.peek() == Token::Comma {
                    self.advance();
                }
                continue;
            }

            let field_span = self.current_span();
            let key = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let value = self.parse_expr()?;
            fields.push(InvoiceField {
                key,
                value,
                span: field_span,
            });

            if *self.peek() == Token::Comma {
                self.advance();
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(Decl::Invoice {
            fields,
            items,
            span,
        })
    }

    fn parse_block(&mut self) -> ParseResult<Vec<Stmt>> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();

        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            let stmt = self.parse_stmt()?;
            stmts.push(stmt);
        }

        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        match self.peek().clone() {
            Token::Let => self.parse_let(false),
            Token::Const => self.parse_let(true),
            Token::Return => {
                let span = self.current_span();
                self.advance();
                let value = self.parse_expr()?;
                Ok(Stmt::Return { value, span })
            }
            Token::Print => {
                let span = self.current_span();
                self.advance();
                self.expect(&Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(Stmt::Print { value, span })
            }
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Ident(name) => {
                let name = name.clone();
                // Check if it's assignment: name = expr
                if *self.peek2() == Token::Assign {
                    let span = self.current_span();
                    self.advance(); // consume ident
                    self.advance(); // consume '='
                    let value = self.parse_expr()?;
                    Ok(Stmt::Assign { name, value, span })
                } else {
                    let expr = self.parse_expr()?;
                    let span = expr.span();
                    Ok(Stmt::ExprStmt { expr, span })
                }
            }
            _ => {
                let expr = self.parse_expr()?;
                let span = expr.span();
                Ok(Stmt::ExprStmt { expr, span })
            }
        }
    }

    fn parse_let(&mut self, is_const: bool) -> ParseResult<Stmt> {
        let span = self.current_span();
        self.advance(); // consume 'let' or 'const'
        let name = self.expect_ident()?;

        let ty = if *self.peek() == Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::Assign)?;
        let value = self.parse_expr()?;

        if is_const {
            Ok(Stmt::Const {
                name,
                ty,
                value,
                span,
            })
        } else {
            Ok(Stmt::Let {
                name,
                ty,
                value,
                span,
            })
        }
    }

    fn parse_if(&mut self) -> ParseResult<Stmt> {
        let span = self.current_span();
        self.advance(); // consume 'if'
        let condition = self.parse_expr()?;
        let then_body = self.parse_block()?;

        let else_body = if *self.peek() == Token::Else {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_body,
            else_body,
            span,
        })
    }

    fn parse_while(&mut self) -> ParseResult<Stmt> {
        let span = self.current_span();
        self.advance(); // consume 'while'
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While {
            condition,
            body,
            span,
        })
    }

    fn parse_for(&mut self) -> ParseResult<Stmt> {
        let span = self.current_span();
        self.advance(); // consume 'for'
        let var = self.expect_ident()?;
        self.expect(&Token::In)?;
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::For {
            var,
            iterable,
            body,
            span,
        })
    }

    // Expression parsing with precedence climbing
    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and()?;
        while *self.peek() == Token::Or {
            let span = self.current_span();
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality()?;
        while *self.peek() == Token::And {
            let span = self.current_span();
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Token::Eq => BinOp::Eq,
                Token::NotEq => BinOp::NotEq,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::Lt => BinOp::Lt,
                Token::LtEq => BinOp::LtEq,
                Token::Gt => BinOp::Gt,
                Token::GtEq => BinOp::GtEq,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        match self.peek().clone() {
            Token::Minus => {
                let span = self.current_span();
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                })
            }
            Token::Not => {
                let span = self.current_span();
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_primary()?;

        while *self.peek() == Token::Dot {
            let span = self.current_span();
            self.advance();
            let field = self.expect_ident()?;
            expr = Expr::FieldAccess {
                object: Box::new(expr),
                field,
                span,
            };
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        match self.peek().clone() {
            Token::Integer(n) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Integer { value: n, span })
            }
            Token::Float(f) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Float { value: f, span })
            }
            Token::StringLit(s) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::StringLit { value: s, span })
            }
            Token::Bool(b) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Bool { value: b, span })
            }
            Token::Nil => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Nil { span })
            }
            Token::Money(v, c) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Money {
                    value: v,
                    currency: c,
                    span,
                })
            }

            Token::LBracket => {
                let span = self.current_span();
                self.advance();
                let items = self.parse_expr_list(Token::RBracket, "]", "item")?;
                self.expect(&Token::RBracket)?;
                Ok(Expr::Array { items, span })
            }

            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            Token::Ident(name) => {
                let name = name.clone();
                let span = self.current_span();
                self.advance();

                // Model instance literal: Customer { name: "Ana" }
                if *self.peek() == Token::LBrace
                    && matches!(self.peek2(), Token::Ident(_))
                    && *self.peek3() == Token::Colon
                {
                    self.advance();
                    let fields = self.parse_object_fields()?;
                    self.expect(&Token::RBrace)?;
                    return Ok(Expr::Object {
                        model: name,
                        fields,
                        span,
                    });
                }

                // Static call: Name::method()
                if *self.peek() == Token::ColonColon {
                    self.advance();
                    let method = self.expect_ident()?;
                    self.expect(&Token::LParen)?;
                    let args = self.parse_expr_list(Token::RParen, ")", "argumento")?;
                    self.expect(&Token::RParen)?;
                    return Ok(Expr::StaticCall {
                        ty: name,
                        method,
                        args,
                        span,
                    });
                }

                // Function call: name(args)
                if *self.peek() == Token::LParen {
                    self.advance();
                    let args = self.parse_expr_list(Token::RParen, ")", "argumento")?;
                    self.expect(&Token::RParen)?;
                    return Ok(Expr::Call { name, args, span });
                }

                Ok(Expr::Ident { name, span })
            }

            _ => {
                let line = self.current_line();
                let column = self.current_column();
                let tok = self.advance();
                Err(self.error_at(line, column, format!("expressão inválida: {:?}", tok)))
            }
        }
    }
}
