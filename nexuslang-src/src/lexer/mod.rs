/// NexusLang Lexer — converte código fonte em tokens
use crate::diagnostic::{codes, Diagnostic};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literais
    Integer(i64),
    Float(f64),
    StringLit(String),
    Bool(bool),
    Money(f64, String), // valor + moeda (kz, usd, eur, ...)
    Nil,

    // Identificadores e palavras-chave
    Ident(String),
    Let,
    Const,
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Model,
    Workflow,
    Step,
    Route,
    Auth,
    Invoice,
    Print,

    // Tipos
    TypeString,
    TypeInt,
    TypeFloat,
    TypeBool,
    TypeMoney,
    TypeDate,

    // HTTP Methods
    Get,
    Post,
    Put,
    Delete,

    // Operadores
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,         // ==
    NotEq,      // !=
    Lt,         // <
    LtEq,       // <=
    Gt,         // >
    GtEq,       // >=
    And,        // &&
    Or,         // ||
    Not,        // !
    Assign,     // =
    Arrow,      // ->
    ColonColon, // ::

    // Delimitadores
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Question,
    Slash2, // /path

    // Especial
    Eof,
    Newline,
}

#[derive(Debug, Clone)]
pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    pub line: usize,
    pub column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied();
        self.pos += 1;
        if ch == Some('\n') {
            self.line += 1;
            self.column = 1;
        } else if ch.is_some() {
            self.column += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        // skip // comment
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn read_string(&mut self, start_line: usize, start_column: usize) -> Result<Token, Diagnostic> {
        let mut s = String::new();
        // opening quote already consumed
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => {
                        return Err(Diagnostic::lexer(
                            "string nao terminada",
                            start_line,
                            start_column,
                        )
                        .with_code(codes::LEXER_UNTERMINATED_STRING))
                    }
                },
                Some(c) => s.push(c),
                None => {
                    return Err(
                        Diagnostic::lexer("string nao terminada", start_line, start_column)
                            .with_code(codes::LEXER_UNTERMINATED_STRING),
                    )
                }
            }
        }
        Ok(Token::StringLit(s))
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut num = String::from(first);
        let mut is_float = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num.push(ch);
                self.advance();
            } else if ch == '.' && self.peek2().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                is_float = true;
                num.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for currency suffix (e.g. "300000 kz")
        // We skip whitespace and check for currency
        let saved_pos = self.pos;
        let saved_line = self.line;
        let saved_column = self.column;

        // skip spaces
        while self.peek() == Some(' ') || self.peek() == Some('\t') {
            self.advance();
        }

        let currencies = ["kz", "usd", "eur", "gbp", "brl", "aoa"];
        let mut currency_found = None;

        for cur in &currencies {
            let chars: Vec<char> = cur.chars().collect();
            let mut matches = true;
            for (i, &c) in chars.iter().enumerate() {
                if self.source.get(self.pos + i).copied() != Some(c) {
                    matches = false;
                    break;
                }
            }
            if matches {
                // make sure it's not followed by alphanumeric
                let after = self.source.get(self.pos + chars.len()).copied();
                if after
                    .map(|c| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(true)
                {
                    currency_found = Some(cur.to_string());
                    for _ in 0..chars.len() {
                        self.advance();
                    }
                    break;
                }
            }
        }

        if let Some(cur) = currency_found {
            let val: f64 = num.parse().unwrap_or(0.0);
            Token::Money(val, cur)
        } else {
            // restore position (no currency found)
            self.pos = saved_pos;
            self.line = saved_line;
            self.column = saved_column;
            if is_float {
                Token::Float(num.parse().unwrap_or(0.0))
            } else {
                Token::Integer(num.parse().unwrap_or(0))
            }
        }
    }

    fn read_ident(&mut self, first: char) -> Token {
        let mut ident = String::from(first);
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        match ident.as_str() {
            "let" => Token::Let,
            "const" => Token::Const,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "model" => Token::Model,
            "workflow" => Token::Workflow,
            "step" => Token::Step,
            "route" => Token::Route,
            "auth" => Token::Auth,
            "invoice" => Token::Invoice,
            "print" => Token::Print,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "nil" => Token::Nil,
            "string" => Token::TypeString,
            "int" => Token::TypeInt,
            "float" => Token::TypeFloat,
            "bool" => Token::TypeBool,
            "money" => Token::TypeMoney,
            "date" => Token::TypeDate,
            "GET" => Token::Get,
            "POST" => Token::Post,
            "PUT" => Token::Put,
            "DELETE" => Token::Delete,
            _ => Token::Ident(ident),
        }
    }

    pub fn tokenize_spanned(&mut self) -> Vec<(Token, usize, usize)> {
        self.tokenize_spanned_diagnostic()
            .unwrap_or_else(|_| vec![(Token::Eof, self.line, self.column)])
    }

    pub fn tokenize_spanned_diagnostic(
        &mut self,
    ) -> Result<Vec<(Token, usize, usize)>, Diagnostic> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();

            let line = self.line;
            let column = self.column;
            let ch = match self.peek() {
                Some(c) => c,
                None => {
                    tokens.push((Token::Eof, line, column));
                    break;
                }
            };

            // Skip comments
            if ch == '/' && self.peek2() == Some('/') {
                self.advance();
                self.advance();
                self.skip_comment();
                continue;
            }

            // Newlines (significant for some contexts)
            if ch == '\n' {
                self.advance();
                // don't push newline tokens — we use statement-based parsing
                continue;
            }

            self.advance();

            let tok = match ch {
                '"' => self.read_string(line, column)?,
                '0'..='9' => self.read_number(ch),
                'a'..='z' | 'A'..='Z' | '_' => self.read_ident(ch),
                '+' => Token::Plus,
                '-' => {
                    if self.peek() == Some('>') {
                        self.advance();
                        Token::Arrow
                    } else {
                        Token::Minus
                    }
                }
                '*' => Token::Star,
                '%' => Token::Percent,
                '=' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::Eq
                    } else {
                        Token::Assign
                    }
                }
                '!' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::NotEq
                    } else {
                        Token::Not
                    }
                }
                '<' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::LtEq
                    } else {
                        Token::Lt
                    }
                }
                '>' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::GtEq
                    } else {
                        Token::Gt
                    }
                }
                '&' => {
                    if self.peek() == Some('&') {
                        self.advance();
                        Token::And
                    } else {
                        return Err(Diagnostic::lexer(
                            "operador '&' invalido; use '&&'",
                            line,
                            column,
                        )
                        .with_code(codes::LEXER_INVALID_OPERATOR));
                    }
                }
                '|' => {
                    if self.peek() == Some('|') {
                        self.advance();
                        Token::Or
                    } else {
                        return Err(Diagnostic::lexer(
                            "operador '|' invalido; use '||'",
                            line,
                            column,
                        )
                        .with_code(codes::LEXER_INVALID_OPERATOR));
                    }
                }
                ':' => {
                    if self.peek() == Some(':') {
                        self.advance();
                        Token::ColonColon
                    } else {
                        Token::Colon
                    }
                }
                '/' => {
                    // Could be a path in route context
                    Token::Slash
                }
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                ',' => Token::Comma,
                ';' => Token::Semicolon,
                '.' => Token::Dot,
                '?' => Token::Question,
                _ => {
                    return Err(Diagnostic::lexer(
                        format!("caractere invalido '{}'", ch),
                        line,
                        column,
                    ))
                }
            };

            tokens.push((tok, line, column));
        }

        Ok(tokens)
    }

    pub fn tokenize(&mut self) -> Vec<(Token, usize)> {
        self.tokenize_spanned()
            .into_iter()
            .map(|(token, line, _)| (token, line))
            .collect()
    }
}
