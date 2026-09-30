use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    Start,

    // one or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // literals.
    Identifier,
    QuotedIdentifier,
    String,
    Number,

    // keywords.
    And,
    Asc,
    Desc,
    Or,
    True,
    False,
    Not,
    Null,
    Select,
    From,
    Inner,
    Left,
    Right,
    Join,
    Full,
    On,
    Offset,
    Where,
    Order,
    By,
    Limit,
    Create,
    Update,
    Delete,
    Alter,
    Eof,
    Table,
    Primary,
    Key,
    Autoincrement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub t_type: TokenType,
    pub start: usize,
    pub length: usize,
    pub line: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScanError {
    message: &'static str,
    line: usize,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[line {}] {}", self.line, self.message)
    }
}

impl std::error::Error for ScanError {}

#[derive(Debug)]
pub struct Scanner<'src> {
    pub source: &'src [u8],
    pub start: usize,
    pub current: usize,
    pub line: usize,
}

impl<'src> Scanner<'src> {
    pub fn init_scanner(src: &'src str) -> Self {
        Scanner {
            source: src.as_bytes(),
            start: 0,
            current: 0,
            line: 1,
        }
    }

    fn advance(&mut self) -> Option<u8> {
        let byte = *self.source.get(self.current)?;
        self.current += 1;
        Some(byte)
    }

    pub fn peek(&self) -> Option<u8> {
        self.source.get(self.current).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.source.get(self.current + 1).copied()
    }

    fn match_next(&mut self, expected: u8) -> bool {
        if self.peek() != Some(expected) {
            return false;
        }

        self.current += 1;

        true
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    pub fn is_eof(token: &Token) -> bool {
        token.t_type == TokenType::Eof
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\r' | b'\t') => {
                    self.advance();
                }
                Some(b'\n') => {
                    self.line += 1;
                    self.advance();
                }
                Some(b'-') if self.peek_next() == Some(b'-') => {
                    self.skip_comments();
                }
                Some(b'#') => {
                    self.skip_comments();
                }
                _ => return,
            }
        }
    }

    fn make_string(&mut self, delimiter: u8, token_type: TokenType) -> Result<Token, ScanError> {
        loop {
            match self.peek() {
                Some(c) if c == delimiter => {
                    if self.peek_next() == Some(delimiter) {
                        self.advance();
                        self.advance();
                        continue;
                    }

                    self.advance();
                    return Ok(self.make_token(token_type));
                }
                Some(b'\n') => {
                    self.line += 1;
                    self.advance();
                }
                Some(_) => {
                    self.advance();
                }
                None => return Err(self.error("Unterminated string.")),
            }
        }
    }

    fn make_quoted_identifier(&mut self, delimiter: u8) -> Result<Token, ScanError> {
        self.make_string(delimiter, TokenType::QuotedIdentifier)
    }

    fn make_quoted_string(&mut self, delimiter: u8) -> Result<Token, ScanError> {
        self.make_string(delimiter, TokenType::String)
    }

    fn make_number(&mut self) -> Token {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.advance();
        }

        if self.peek() == Some(b'.') && matches!(self.peek_next(), Some(b'0'..=b'9')) {
            self.advance();

            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.advance();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn check_keyword(
        &self,
        start: usize,
        length: usize,
        rest: &str,
        token_type: TokenType,
    ) -> TokenType {
        if (self.current - self.start) == (start + length) {
            let from = self.start + start;
            let to = from + length;

            if self
                .source
                .get(from..to)
                .is_some_and(|c| c.eq_ignore_ascii_case(rest.as_bytes()))
            {
                return token_type;
            }
        }

        TokenType::Identifier
    }

    fn identifier_type(&self) -> TokenType {
        match self.source.get(self.start).map(u8::to_ascii_lowercase) {
            Some(b'a') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'n') => self.check_keyword(1, 2, "nd", TokenType::And),
                        Some(b'l') => self.check_keyword(1, 4, "lter", TokenType::Alter),
                        Some(b's') => self.check_keyword(1, 2, "sc", TokenType::Asc),
                        Some(b'u') => {
                            self.check_keyword(1, 12, "utoincrement", TokenType::Autoincrement)
                        }
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'b') => self.check_keyword(1, 1, "y", TokenType::By),

            Some(b'c') => self.check_keyword(1, 5, "reate", TokenType::Create),

            Some(b'd') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'e') => {
                            if self.current - self.start > 2 {
                                match self.source.get(self.start + 2).map(u8::to_ascii_lowercase) {
                                    Some(b'l') => {
                                        self.check_keyword(1, 5, "elete", TokenType::Delete)
                                    }
                                    Some(b's') => self.check_keyword(1, 3, "esc", TokenType::Desc),
                                    _ => TokenType::Identifier,
                                }
                            } else {
                                TokenType::Identifier
                            }
                        }
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'f') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'a') => self.check_keyword(1, 4, "alse", TokenType::False),
                        Some(b'r') => self.check_keyword(1, 3, "rom", TokenType::From),
                        Some(b'u') => self.check_keyword(1, 3, "ull", TokenType::Full),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'j') => self.check_keyword(1, 3, "oin", TokenType::Join),

            Some(b'i') => self.check_keyword(1, 4, "nner", TokenType::Inner),

            Some(b'k') => self.check_keyword(1, 2, "ey", TokenType::Key),

            Some(b'l') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'e') => self.check_keyword(1, 3, "eft", TokenType::Left),
                        Some(b'i') => self.check_keyword(1, 4, "imit", TokenType::Limit),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'n') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'o') => self.check_keyword(1, 2, "ot", TokenType::Not),
                        Some(b'u') => self.check_keyword(1, 3, "ull", TokenType::Null),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'o') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'f') => self.check_keyword(1, 5, "ffset", TokenType::Offset),
                        Some(b'n') => self.check_keyword(1, 1, "n", TokenType::On),
                        Some(b'r') => {
                            if self.current - self.start > 2 {
                                match self.source.get(self.start + 2).map(u8::to_ascii_lowercase) {
                                    Some(b'd') => {
                                        self.check_keyword(1, 4, "rder", TokenType::Order)
                                    }
                                    _ => TokenType::Identifier,
                                }
                            } else {
                                TokenType::Or
                            }
                        }
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'p') => self.check_keyword(1, 6, "rimary", TokenType::Primary),

            Some(b'r') => self.check_keyword(1, 4, "ight", TokenType::Right),

            Some(b's') => self.check_keyword(1, 5, "elect", TokenType::Select),

            Some(b't') => {
                if self.current - self.start > 1 {
                    match self.source.get(self.start + 1).map(u8::to_ascii_lowercase) {
                        Some(b'a') => self.check_keyword(1, 4, "able", TokenType::Table),
                        Some(b'r') => self.check_keyword(1, 3, "rue", TokenType::True),
                        _ => TokenType::Identifier,
                    }
                } else {
                    TokenType::Identifier
                }
            }

            Some(b'u') => self.check_keyword(1, 5, "pdate", TokenType::Update),

            Some(b'w') => self.check_keyword(1, 4, "here", TokenType::Where),

            _ => TokenType::Identifier,
        }
    }

    fn make_identifier(&mut self) -> Token {
        while matches!(
            self.peek(),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
        ) {
            self.advance();
        }

        let token_type = self.identifier_type();
        self.make_token(token_type)
    }

    fn skip_comments(&mut self) {
        while !matches!(self.peek(), Some(b'\n') | None) {
            self.advance();
        }
    }

    fn error(&self, message: &'static str) -> ScanError {
        ScanError {
            message,
            line: self.line,
        }
    }

    fn make_token(&self, t_type: TokenType) -> Token {
        Token {
            t_type,
            start: self.start,
            length: self.current - self.start,
            line: self.line,
        }
    }

    pub fn scan_token(&mut self) -> Result<Token, ScanError> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_at_end() {
            return Ok(self.make_token(TokenType::Eof));
        }

        let byte = self.advance();

        match byte {
            Some(b'0'..=b'9') => Ok(self.make_number()),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'_') => Ok(self.make_identifier()),
            Some(b'(') => Ok(self.make_token(TokenType::LeftParen)),
            Some(b')') => Ok(self.make_token(TokenType::RightParen)),
            Some(b'{') => Ok(self.make_token(TokenType::LeftBrace)),
            Some(b'}') => Ok(self.make_token(TokenType::RightBrace)),
            Some(b';') => Ok(self.make_token(TokenType::Semicolon)),
            Some(b',') => Ok(self.make_token(TokenType::Comma)),
            Some(b'.') => Ok(self.make_token(TokenType::Dot)),
            Some(b'-') => Ok(self.make_token(TokenType::Minus)),
            Some(b'+') => Ok(self.make_token(TokenType::Plus)),
            Some(b'/') => Ok(self.make_token(TokenType::Slash)),
            Some(b'*') => Ok(self.make_token(TokenType::Star)),

            Some(b'!') => {
                let token_type = if self.match_next(b'=') {
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };

                Ok(self.make_token(token_type))
            }
            Some(b'=') => {
                let token_type = if self.match_next(b'=') {
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };

                Ok(self.make_token(token_type))
            }
            Some(b'<') => {
                let token_type = if self.match_next(b'=') {
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };

                Ok(self.make_token(token_type))
            }
            Some(b'>') => {
                let token_type = if self.match_next(b'=') {
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                };

                Ok(self.make_token(token_type))
            }

            Some(b'"') => self.make_quoted_identifier(b'"'),
            Some(b'\'') => self.make_quoted_string(b'\''),

            _ => Err(self.error("Unexpected character.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("(", TokenType::LeftParen, 0, 1, 1)]
    #[case("   (", TokenType::LeftParen, 3, 1, 1)]
    #[case("\n(", TokenType::LeftParen, 1, 1, 2)]
    #[case("\n\n(", TokenType::LeftParen, 2, 1, 3)]
    fn scans_left_parenthesis(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case(")", TokenType::RightParen, 0, 1, 1)]
    #[case("   )", TokenType::RightParen, 3, 1, 1)]
    #[case("\n)", TokenType::RightParen, 1, 1, 2)]
    #[case("\n\n)", TokenType::RightParen, 2, 1, 3)]
    fn scans_right_parenthesis(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case("!=", TokenType::BangEqual, 0, 2, 1)]
    #[case("   !=", TokenType::BangEqual, 3, 2, 1)]
    #[case("\n!=", TokenType::BangEqual, 1, 2, 2)]
    #[case("\n\n!=", TokenType::BangEqual, 2, 2, 3)]
    fn scans_bang_equal(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case("!", TokenType::Bang, 0, 1, 1)]
    #[case("   !", TokenType::Bang, 3, 1, 1)]
    #[case("\n!", TokenType::Bang, 1, 1, 2)]
    #[case("\n\n!", TokenType::Bang, 2, 1, 3)]
    fn scans_bang(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case(r#""hello""#, TokenType::QuotedIdentifier, 0, 7, 1)]
    #[case(r#""it's valid""#, TokenType::QuotedIdentifier, 0, 12, 1)]
    #[case(r#"   "hello""#, TokenType::QuotedIdentifier, 3, 7, 1)]
    #[case("\"hello\nworld\"", TokenType::QuotedIdentifier, 0, 13, 2)]
    fn scans_strings(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[test]
    fn reports_unexpected_character() {
        let mut scanner = Scanner::init_scanner("@");

        let error = scanner.scan_token().unwrap_err();

        assert_eq!(error.message, "Unexpected character.");
        assert_eq!(error.line, 1);
    }

    #[rstest]
    #[case(r#""unterminated"#, 1)]
    #[case("'unterminated", 1)]
    #[case("'unterminated\nstring", 2)]
    fn reports_unterminated_strings(#[case] source: &str, #[case] expected_line: usize) {
        let mut scanner = Scanner::init_scanner(source);

        let error = scanner.scan_token().unwrap_err();

        assert_eq!(error.message, "Unterminated string.");
        assert_eq!(error.line, expected_line);
    }

    #[rstest]
    #[case(r#""hello""#, r#""hello""#)]
    #[case("'hello'", "'hello'")]
    fn string_token_covers_the_complete_lexeme(
        #[case] source: &str,
        #[case] expected_lexeme: &str,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        let lexeme = &source[token.start..token.start + token.length];

        assert_eq!(lexeme, expected_lexeme);
    }

    #[rstest]
    #[case("0", TokenType::Number, 0, 1)]
    #[case("123", TokenType::Number, 0, 3)]
    #[case("123.45", TokenType::Number, 0, 6)]
    #[case("   123.45", TokenType::Number, 3, 6)]
    #[case("123.", TokenType::Number, 0, 3)]
    #[case("123.4.5", TokenType::Number, 0, 5)]
    fn scans_numbers(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
    }

    #[rstest]
    #[case("and", TokenType::And, 0, 3, 1)]
    #[case("   and", TokenType::And, 3, 3, 1)]
    #[case("\nand", TokenType::And, 1, 3, 2)]
    #[case("an", TokenType::Identifier, 0, 2, 1)]
    #[case("anderson", TokenType::Identifier, 0, 8, 1)]
    #[case("android", TokenType::Identifier, 0, 7, 1)]
    fn scans_and_keyword(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case("select", TokenType::Select, 0, 6, 1)]
    #[case("   select", TokenType::Select, 3, 6, 1)]
    #[case("\nselect", TokenType::Select, 1, 6, 2)]
    #[case("se", TokenType::Identifier, 0, 2, 1)]
    #[case("selection", TokenType::Identifier, 0, 9, 1)]
    fn scans_select_keyword(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case("left", TokenType::Left, 0, 4, 1)]
    #[case("   left", TokenType::Left, 3, 4, 1)]
    #[case("\nleft", TokenType::Left, 1, 4, 2)]
    #[case("le", TokenType::Identifier, 0, 2, 1)]
    #[case("leftion", TokenType::Identifier, 0, 7, 1)]
    fn scans_left_keyword(
        #[case] source: &str,
        #[case] expected_type: TokenType,
        #[case] expected_start: usize,
        #[case] expected_length: usize,
        #[case] expected_line: usize,
    ) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, expected_type);
        assert_eq!(token.start, expected_start);
        assert_eq!(token.length, expected_length);
        assert_eq!(token.line, expected_line);
    }

    #[rstest]
    #[case("-- comment\n", 2)]
    #[case("# comment\n", 2)]
    #[case("-- comment without newline", 1)]
    #[case("# comment without newline", 1)]
    fn skips_comments(#[case] source: &str, #[case] expected_line: usize) {
        let mut scanner = Scanner::init_scanner(source);

        let token = scanner.scan_token().unwrap();

        assert_eq!(token.t_type, TokenType::Eof);
        assert_eq!(token.line, expected_line);
    }
}
