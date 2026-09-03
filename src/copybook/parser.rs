use super::ast::{CopybookAst, DataEntry, EntryKind, Picture, PictureKind, Usage};
use super::is_valid_data_name;
use super::normalize::{NormalizedCopybook, normalize_fixed_format};
use crate::error::{CopybookDiagnostic, DiagnosticKind, SourceSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Word(String),
    Period,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
}

impl Token {
    fn text(&self) -> &str {
        match &self.kind {
            TokenKind::Word(word) => word,
            TokenKind::Period => ".",
        }
    }

    fn is_word(&self, expected: &str) -> bool {
        matches!(&self.kind, TokenKind::Word(word) if word.eq_ignore_ascii_case(expected))
    }
}

pub fn parse_copybook(source: &str) -> Result<CopybookAst, CopybookDiagnostic> {
    let normalized = normalize_fixed_format(source)?;
    let tokens = lex(&normalized);
    Parser::new(tokens).parse()
}

fn lex(source: &NormalizedCopybook) -> Vec<Token> {
    let mut tokens = Vec::new();

    for line in &source.lines {
        let bytes = line.text.as_bytes();
        let mut cursor = 0;

        while cursor < bytes.len() {
            if bytes[cursor] == b' ' {
                cursor += 1;
                continue;
            }

            let span = SourceSpan::new(line.source_line, line.source_column + cursor);
            if bytes[cursor] == b'.' {
                tokens.push(Token {
                    kind: TokenKind::Period,
                    span,
                });
                cursor += 1;
                continue;
            }

            let start = cursor;
            while cursor < bytes.len() && bytes[cursor] != b' ' && bytes[cursor] != b'.' {
                cursor += 1;
            }

            tokens.push(Token {
                kind: TokenKind::Word(line.text[start..cursor].to_owned()),
                span,
            });
        }
    }

    tokens
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn parse(mut self) -> Result<CopybookAst, CopybookDiagnostic> {
        let mut entries = Vec::new();
        while self.peek().is_some() {
            entries.push(self.parse_entry()?);
        }
        Ok(CopybookAst { entries })
    }

    fn parse_entry(&mut self) -> Result<DataEntry, CopybookDiagnostic> {
        let level_token = self.next().ok_or_else(|| {
            CopybookDiagnostic::new(
                self.eof_span(),
                DiagnosticKind::UnexpectedEnd {
                    expected: "a two-digit level number".into(),
                },
            )
        })?;
        let level = parse_level(&level_token)?;

        let name_token = self.next().ok_or_else(|| {
            CopybookDiagnostic::new(
                level_token.span,
                DiagnosticKind::UnexpectedEnd {
                    expected: "a data-name".into(),
                },
            )
        })?;
        let name = parse_name(&name_token)?;

        let Some(next) = self.peek() else {
            return Err(CopybookDiagnostic::new(
                self.eof_span(),
                DiagnosticKind::MissingPeriod,
            ));
        };

        let entry = if matches!(next.kind, TokenKind::Period) {
            self.cursor += 1;
            EntryKind::Group
        } else if next.is_word("PIC") || next.is_word("PICTURE") {
            self.cursor += 1;
            self.parse_elementary()?
        } else if looks_like_level(next) {
            return Err(CopybookDiagnostic::new(
                next.span,
                DiagnosticKind::MissingPeriod,
            ));
        } else {
            return Err(unsupported_clause(next));
        };

        Ok(DataEntry {
            level,
            name,
            entry,
            span: level_token.span,
        })
    }

    fn parse_elementary(&mut self) -> Result<EntryKind, CopybookDiagnostic> {
        if self.peek().is_some_and(|token| token.is_word("IS")) {
            self.cursor += 1;
        }

        let picture_token = self.next().ok_or_else(|| {
            CopybookDiagnostic::new(
                self.eof_span(),
                DiagnosticKind::UnexpectedEnd {
                    expected: "a supported PICTURE followed by a period".into(),
                },
            )
        })?;

        if matches!(picture_token.kind, TokenKind::Period) {
            return Err(CopybookDiagnostic::new(
                picture_token.span,
                DiagnosticKind::UnexpectedToken {
                    found: ".".into(),
                    expected: "a supported PICTURE".into(),
                },
            ));
        }
        let picture = parse_picture(&picture_token)?;

        let usage = match self.peek() {
            Some(token) if matches!(token.kind, TokenKind::Period) => Usage::Display,
            Some(token) if token.is_word("USAGE") => {
                self.cursor += 1;
                if self.peek().is_some_and(|token| token.is_word("IS")) {
                    self.cursor += 1;
                }
                self.parse_usage()?
            }
            Some(token) => match usage_from_token(token) {
                Some(usage) => {
                    self.cursor += 1;
                    usage
                }
                None if looks_like_level(token) => {
                    return Err(CopybookDiagnostic::new(
                        token.span,
                        DiagnosticKind::MissingPeriod,
                    ));
                }
                None => return Err(unsupported_clause(token)),
            },
            None => {
                return Err(CopybookDiagnostic::new(
                    self.eof_span(),
                    DiagnosticKind::MissingPeriod,
                ));
            }
        };

        match self.peek() {
            Some(token) if matches!(token.kind, TokenKind::Period) => {
                self.cursor += 1;
            }
            Some(token) if looks_like_level(token) => {
                return Err(CopybookDiagnostic::new(
                    token.span,
                    DiagnosticKind::MissingPeriod,
                ));
            }
            Some(token) => return Err(unsupported_clause(token)),
            None => {
                return Err(CopybookDiagnostic::new(
                    self.eof_span(),
                    DiagnosticKind::MissingPeriod,
                ));
            }
        }

        Ok(EntryKind::Elementary { picture, usage })
    }

    fn parse_usage(&mut self) -> Result<Usage, CopybookDiagnostic> {
        let token = self.next().ok_or_else(|| {
            CopybookDiagnostic::new(
                self.eof_span(),
                DiagnosticKind::UnexpectedEnd {
                    expected: "DISPLAY, COMP, COMP-4, BINARY, COMP-3 or PACKED-DECIMAL".into(),
                },
            )
        })?;

        if matches!(token.kind, TokenKind::Period) {
            return Err(CopybookDiagnostic::new(
                token.span,
                DiagnosticKind::UnexpectedToken {
                    found: ".".into(),
                    expected: "a supported USAGE".into(),
                },
            ));
        }

        usage_from_token(&token).ok_or_else(|| unsupported_clause(&token))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor).cloned();
        if token.is_some() {
            self.cursor += 1;
        }
        token
    }

    fn eof_span(&self) -> SourceSpan {
        self.tokens.last().map_or(SourceSpan::new(1, 1), |token| {
            SourceSpan::new(token.span.line, token.span.column + token.text().len())
        })
    }
}

fn parse_level(token: &Token) -> Result<u8, CopybookDiagnostic> {
    let TokenKind::Word(value) = &token.kind else {
        return Err(CopybookDiagnostic::new(
            token.span,
            DiagnosticKind::InvalidLevel {
                value: token.text().into(),
            },
        ));
    };

    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CopybookDiagnostic::new(
            token.span,
            DiagnosticKind::InvalidLevel {
                value: value.clone(),
            },
        ));
    }

    let level = (value.as_bytes()[0] - b'0') * 10 + (value.as_bytes()[1] - b'0');
    if !(1..=49).contains(&level) {
        return Err(CopybookDiagnostic::new(
            token.span,
            DiagnosticKind::InvalidLevel {
                value: value.clone(),
            },
        ));
    }

    Ok(level)
}

fn parse_name(token: &Token) -> Result<String, CopybookDiagnostic> {
    let TokenKind::Word(value) = &token.kind else {
        return Err(CopybookDiagnostic::new(
            token.span,
            DiagnosticKind::UnexpectedToken {
                found: token.text().into(),
                expected: "a data-name".into(),
            },
        ));
    };

    if !is_valid_data_name(value) {
        return Err(CopybookDiagnostic::new(
            token.span,
            DiagnosticKind::InvalidName {
                value: value.clone(),
            },
        ));
    }

    Ok(value.clone())
}

fn parse_picture(token: &Token) -> Result<Picture, CopybookDiagnostic> {
    let TokenKind::Word(value) = &token.kind else {
        return Err(invalid_picture(
            token,
            "expected an alphanumeric or numeric PICTURE",
        ));
    };

    let uppercase = value.to_ascii_uppercase();
    let (signed, body) = match uppercase.strip_prefix('S') {
        Some(body) => (true, body),
        None => (false, uppercase.as_str()),
    };

    if let Some(length) = parse_symbol_count(body, 'X') {
        if signed {
            return Err(invalid_picture(
                token,
                "the S prefix is only valid for numeric PICTUREs",
            ));
        }
        return Ok(Picture {
            kind: PictureKind::Alphanumeric { length },
            signed: false,
        });
    }

    let numeric_parts: Vec<&str> = body.split('V').collect();
    let (integer_digits, fractional_digits) = match numeric_parts.as_slice() {
        [integer] => (parse_numeric_count(integer), Some(0)),
        [integer, fractional] if is_repeated_nine(integer) && is_repeated_nine(fractional) => (
            parse_numeric_count(integer),
            parse_numeric_count(fractional),
        ),
        _ => (None, None),
    };

    let (Some(integer_digits), Some(fractional_digits)) = (integer_digits, fractional_digits)
    else {
        return Err(invalid_picture(
            token,
            "supported forms are X, X(n), 9, 9(n), 9(n)V9(m), optionally prefixed by S for numeric fields",
        ));
    };

    let integer_digits = u8::try_from(integer_digits)
        .map_err(|_| invalid_picture(token, "integer digit count does not fit in the AST"))?;
    let fractional_digits = u8::try_from(fractional_digits)
        .map_err(|_| invalid_picture(token, "fractional digit count does not fit in the AST"))?;

    Ok(Picture {
        kind: PictureKind::Numeric {
            integer_digits,
            fractional_digits,
        },
        signed,
    })
}

fn parse_numeric_count(value: &str) -> Option<usize> {
    parse_symbol_count(value, '9')
}

fn is_repeated_nine(value: &str) -> bool {
    value.starts_with("9(") && value.ends_with(')')
}

fn parse_symbol_count(value: &str, symbol: char) -> Option<usize> {
    let single = symbol.to_string();
    if value == single {
        return Some(1);
    }

    let repetition = value
        .strip_prefix(&format!("{symbol}("))?
        .strip_suffix(')')?;
    if repetition.is_empty() || !repetition.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let count = repetition.parse::<usize>().ok()?;
    (count > 0).then_some(count)
}

fn usage_from_token(token: &Token) -> Option<Usage> {
    if token.is_word("DISPLAY") {
        Some(Usage::Display)
    } else if token.is_word("COMP") || token.is_word("COMP-4") || token.is_word("BINARY") {
        Some(Usage::Binary)
    } else if token.is_word("COMP-3") || token.is_word("PACKED-DECIMAL") {
        Some(Usage::PackedDecimal)
    } else {
        None
    }
}

fn looks_like_level(token: &Token) -> bool {
    matches!(&token.kind, TokenKind::Word(word) if word.len() == 2 && word.bytes().all(|byte| byte.is_ascii_digit()))
}

fn invalid_picture(token: &Token, details: impl Into<String>) -> CopybookDiagnostic {
    CopybookDiagnostic::new(
        token.span,
        DiagnosticKind::InvalidPicture {
            value: token.text().into(),
            details: details.into(),
        },
    )
}

fn unsupported_clause(token: &Token) -> CopybookDiagnostic {
    CopybookDiagnostic::new(
        token.span,
        DiagnosticKind::UnsupportedClause {
            clause: token.text().into(),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    fn fixed(code: &str) -> String {
        code.lines()
            .map(|line| format!("       {line}\n"))
            .collect()
    }

    #[test]
    fn parses_small_copybook_into_expected_ast() {
        let source = fixed(
            "01 CUSTOMER.\n  05 CUSTOMER-NAME PIC X(12).\n  05 BALANCE PIC S9(7)V9(2) COMP-3.\n  05 FLAGS PIC 9(4) USAGE IS BINARY.",
        );

        let ast = parse_copybook(&source).unwrap();
        assert_eq!(
            ast,
            CopybookAst {
                entries: vec![
                    DataEntry {
                        level: 1,
                        name: "CUSTOMER".into(),
                        entry: EntryKind::Group,
                        span: SourceSpan::new(1, 8),
                    },
                    DataEntry {
                        level: 5,
                        name: "CUSTOMER-NAME".into(),
                        entry: EntryKind::Elementary {
                            picture: Picture {
                                kind: PictureKind::Alphanumeric { length: 12 },
                                signed: false,
                            },
                            usage: Usage::Display,
                        },
                        span: SourceSpan::new(2, 10),
                    },
                    DataEntry {
                        level: 5,
                        name: "BALANCE".into(),
                        entry: EntryKind::Elementary {
                            picture: Picture {
                                kind: PictureKind::Numeric {
                                    integer_digits: 7,
                                    fractional_digits: 2,
                                },
                                signed: true,
                            },
                            usage: Usage::PackedDecimal,
                        },
                        span: SourceSpan::new(3, 10),
                    },
                    DataEntry {
                        level: 5,
                        name: "FLAGS".into(),
                        entry: EntryKind::Elementary {
                            picture: Picture {
                                kind: PictureKind::Numeric {
                                    integer_digits: 4,
                                    fractional_digits: 0,
                                },
                                signed: false,
                            },
                            usage: Usage::Binary,
                        },
                        span: SourceSpan::new(4, 10),
                    },
                ],
            }
        );
    }

    #[test]
    fn grammar_is_case_insensitive_and_names_retain_spelling() {
        let ast = parse_copybook(&fixed(
            "01 lower-record.\n  05 text-field picture is x(3) usage is display.\n  05 packed picture s9(2)v9(1) packed-decimal.",
        ))
        .unwrap();

        assert_eq!(ast.entries[0].name, "lower-record");
        assert!(matches!(
            ast.entries[1].entry,
            EntryKind::Elementary {
                usage: Usage::Display,
                ..
            }
        ));
        assert!(matches!(
            ast.entries[2].entry,
            EntryKind::Elementary {
                usage: Usage::PackedDecimal,
                ..
            }
        ));
    }

    #[test]
    fn accepts_a_statement_split_across_fixed_format_lines() {
        let source =
            fixed("01 RECORD.\n  05 AMOUNT\n     PICTURE IS S9(8)V9(2)\n     USAGE IS COMP-3.");
        let ast = parse_copybook(&source).unwrap();

        assert_eq!(ast.entries.len(), 2);
        assert!(matches!(
            ast.entries[1].entry,
            EntryKind::Elementary {
                picture: Picture {
                    kind: PictureKind::Numeric {
                        integer_digits: 8,
                        fractional_digits: 2
                    },
                    signed: true,
                },
                usage: Usage::PackedDecimal,
            }
        ));
    }

    #[test]
    fn supports_all_usage_aliases() {
        for (spelling, expected) in [
            ("DISPLAY", Usage::Display),
            ("COMP", Usage::Binary),
            ("COMP-4", Usage::Binary),
            ("BINARY", Usage::Binary),
            ("COMP-3", Usage::PackedDecimal),
            ("PACKED-DECIMAL", Usage::PackedDecimal),
        ] {
            let source = fixed(&format!("01 RECORD.\n  05 VALUE PIC 9(4) {spelling}."));
            let ast = parse_copybook(&source).unwrap();
            assert!(matches!(
                ast.entries[1].entry,
                EntryKind::Elementary { usage, .. } if usage == expected
            ));
        }
    }

    #[test]
    fn rejects_malformed_pictures() {
        for picture in [
            "X(0)",
            "X()",
            "SX(2)",
            "99",
            "9V9",
            "9(2)V9",
            "9(2)V9(0)",
            "S",
            "9(2)A",
        ] {
            let source = fixed(&format!("01 RECORD.\n  05 VALUE PIC {picture}."));
            let error = parse_copybook(&source).unwrap_err();
            assert!(
                matches!(error.kind, DiagnosticKind::InvalidPicture { .. }),
                "picture {picture:?} produced {error:?}"
            );
        }
    }

    #[test]
    fn rejects_every_unsupported_clause_explicitly() {
        for clause in [
            "OCCURS",
            "REDEFINES",
            "VALUE",
            "SYNC",
            "JUSTIFIED",
            "SIGN",
            "COMP-5",
        ] {
            let source = if clause == "REDEFINES" {
                fixed("01 RECORD.\n  05 VALUE REDEFINES OTHER PIC X.")
            } else {
                fixed(&format!("01 RECORD.\n  05 VALUE PIC X {clause}."))
            };
            let error = parse_copybook(&source).unwrap_err();
            assert!(
                matches!(
                    error.kind,
                    DiagnosticKind::UnsupportedClause { clause: ref found } if found.eq_ignore_ascii_case(clause)
                ),
                "clause {clause:?} produced {error:?}"
            );
        }
    }

    #[test]
    fn a_group_with_a_clause_is_not_silently_accepted() {
        let error = parse_copybook(&fixed("01 RECORD OCCURS 2.")).unwrap_err();
        assert_eq!(
            error.kind,
            DiagnosticKind::UnsupportedClause {
                clause: "OCCURS".into()
            }
        );
    }

    #[test]
    fn reports_missing_period() {
        let error = parse_copybook(&fixed("01 RECORD.\n  05 VALUE PIC X(3)")).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::MissingPeriod);
    }

    #[test]
    fn truncated_usage_points_to_the_end_of_the_original_line() {
        let error = parse_copybook(&fixed("01 RECORD.\n05 VALUE PIC 9 USAGE IS")).unwrap_err();

        assert_eq!(error.span, SourceSpan::new(2, 31));
        assert!(matches!(error.kind, DiagnosticKind::UnexpectedEnd { .. }));
    }

    #[test]
    fn diagnostics_include_original_line_and_column() {
        let error = parse_copybook(&fixed("01 RECORD.\n  05 VALUE PIC X(0).")).unwrap_err();
        assert_eq!(error.span, SourceSpan::new(2, 23));
        assert!(error.to_string().starts_with("line 2, column 23:"));
    }

    #[test]
    fn deterministic_arbitrary_invalid_sources_never_panic() {
        let mut state = 0x4d_32_43_u64;
        for length in 0..256 {
            let mut code = String::new();
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let byte = 0x20 + ((state >> 32) % 95) as u8;
                code.push(char::from(byte));
            }
            let source = fixed(&code);
            let result = catch_unwind(AssertUnwindSafe(|| parse_copybook(&source)));
            assert!(result.is_ok(), "parser panicked for {code:?}");
        }
    }
}
