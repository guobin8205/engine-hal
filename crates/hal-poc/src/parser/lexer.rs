//! 词法器，对应 Godot `VariantParser::get_token`。
//!
//! 移植自 `core/variant/variant_parser.cpp:162-520`。
//! 把字符流转成 Token 流。

use glam::Vec4;

use crate::parser::stream::Stream;
use crate::types::Variant;

/// Token 类型，对应 Godot `VariantParser::TokenType`。
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    CurlyOpen,    // {
    CurlyClose,   // }
    BracketOpen,  // [
    BracketClose, // ]
    ParenOpen,    // (
    ParenClose,   // )
    Identifier,
    String,
    StringName,
    Number,
    Color, // #RRGGBBAA 词法层
    Colon,      // :
    Comma,      // ,
    Period,     // .
    Equal,      // =
    Eof,
    Error,
}

/// 一个词法 token。value 携带具体数据（仅对部分类型有意义）。
#[derive(Debug, Clone)]
pub struct Token {
    pub ty: TokenType,
    pub value: Variant,
    /// 原始字符串形式（用于标识符等场景，便于错误信息）
    pub raw: String,
}

impl Token {
    fn simple(ty: TokenType) -> Self {
        Token {
            ty,
            value: Variant::Null,
            raw: String::new(),
        }
    }
}

/// 词法错误：附行列信息。
#[derive(Debug, Clone)]
pub struct LexError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for LexError {}

/// 读一个 token。对应 Godot `get_token`。
pub fn get_token(stream: &mut Stream) -> Result<Token, LexError> {
    loop {
        let cchar = stream.get_char();
        if cchar == '\0' && stream.is_eof() {
            return Ok(Token::simple(TokenType::Eof));
        }

        match cchar {
            // 换行和空白被跳过（Godot 里 \n 增加 line 但不产 token）
            '\n' | '\r' => continue,
            '{' => return Ok(Token::simple(TokenType::CurlyOpen)),
            '}' => return Ok(Token::simple(TokenType::CurlyClose)),
            '[' => return Ok(Token::simple(TokenType::BracketOpen)),
            ']' => return Ok(Token::simple(TokenType::BracketClose)),
            '(' => return Ok(Token::simple(TokenType::ParenOpen)),
            ')' => return Ok(Token::simple(TokenType::ParenClose)),
            ':' => return Ok(Token::simple(TokenType::Colon)),
            ';' => {
                // 行注释（Godot .tscn 里少见，但支持）
                loop {
                    let ch = stream.get_char();
                    if ch == '\0' && stream.is_eof() {
                        return Ok(Token::simple(TokenType::Eof));
                    }
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }
            ',' => return Ok(Token::simple(TokenType::Comma)),
            '.' => return Ok(Token::simple(TokenType::Period)),
            '=' => return Ok(Token::simple(TokenType::Equal)),
            '#' => {
                // Color hex 词法：# 后跟 hex 数字序列
                let mut s = String::from("#");
                loop {
                    let ch = stream.get_char();
                    if ch == '\0' && stream.is_eof() {
                        return Ok(Token {
                            ty: TokenType::Eof,
                            value: Variant::Null,
                            raw: s,
                        });
                    }
                    if is_hex_digit(ch) {
                        s.push(ch);
                    } else {
                        stream.saved = ch;
                        break;
                    }
                }
                let color = parse_color_hex(&s);
                return Ok(Token {
                    ty: TokenType::Color,
                    value: color,
                    raw: s,
                });
            }
            '&' => {
                // StringName 前缀 &"，后面必须跟 "
                let next = stream.get_char();
                if next != '"' {
                    return Err(LexError {
                        line: stream.line,
                        message: "Expected '\"' after '&'".into(),
                    });
                }
                // 直接调 parse_string，is_string_name=true
                return parse_string(stream, true, stream.line);
            }
            '"' => {
                return parse_string(stream, false, stream.line);
            }
            c if c <= ' ' => {
                // 其他空白（空格、制表符等），跳过
                continue;
            }
            _ => {
                // 标识符或数字
                return parse_identifier_or_number(stream, cchar);
            }
        }
    }
}

/// 解析字符串字面量（处理转义）。移植自 Godot `get_token` 的 '"' 分支。
fn parse_string(stream: &mut Stream, is_string_name: bool, _start_line: usize) -> Result<Token, LexError> {
    let mut s = String::new();
    let mut prev_surrogate: u32 = 0; // UTF-16 代理对处理

    loop {
        let ch = stream.get_char();
        if ch == '\0' && stream.is_eof() {
            return Err(LexError {
                line: stream.line,
                message: "Unterminated string".into(),
            });
        }
        if ch == '"' {
            break;
        }
        if ch == '\\' {
            // 转义序列
            let next = stream.get_char();
            if next == '\0' && stream.is_eof() {
                return Err(LexError {
                    line: stream.line,
                    message: "Unterminated string".into(),
                });
            }
            let res: char = match next {
                'b' => '\u{0008}',
                't' => '\t',
                'n' => '\n',
                'f' => '\u{000C}',
                'r' => '\r',
                'u' | 'U' => {
                    let hex_len = if next == 'U' { 6 } else { 4 };
                    let mut value: u32 = 0;
                    for _ in 0..hex_len {
                        let c = stream.get_char();
                        if c == '\0' && stream.is_eof() {
                            return Err(LexError {
                                line: stream.line,
                                message: "Unterminated string".into(),
                            });
                        }
                        if !is_hex_digit(c) {
                            return Err(LexError {
                                line: stream.line,
                                message: "Malformed hex constant in string".into(),
                            });
                        }
                        value = (value << 4) | hex_value(c);
                    }
                    // UTF-16 代理对处理
                    if (value & 0xfffffc00) == 0xd800 {
                        if prev_surrogate != 0 {
                            return Err(LexError {
                                line: stream.line,
                                message: "Invalid UTF-16 sequence, unpaired lead surrogate".into(),
                            });
                        }
                        prev_surrogate = value;
                        continue;
                    } else if (value & 0xfffffc00) == 0xdc00 {
                        if prev_surrogate == 0 {
                            return Err(LexError {
                                line: stream.line,
                                message: "Invalid UTF-16 sequence, unpaired trail surrogate".into(),
                            });
                        }
                        let combined = (prev_surrogate << 10) + value - ((0xd800 << 10) + 0xdc00 - 0x10000);
                        prev_surrogate = 0;
                        char::from_u32(combined).unwrap_or('\u{FFFD}')
                    } else {
                        char::from_u32(value).unwrap_or('\u{FFFD}')
                    }
                }
                // 未知转义：保留字面量（Godot default 分支）
                other => other,
            };

            if prev_surrogate != 0 {
                return Err(LexError {
                    line: stream.line,
                    message: "Invalid UTF-16 sequence, unpaired lead surrogate".into(),
                });
            }
            s.push(res);
        } else {
            if prev_surrogate != 0 {
                return Err(LexError {
                    line: stream.line,
                    message: "Invalid UTF-16 sequence, unpaired lead surrogate".into(),
                });
            }
            s.push(ch);
        }
    }

    if prev_surrogate != 0 {
        return Err(LexError {
            line: stream.line,
            message: "Invalid UTF-16 sequence, unpaired lead surrogate".into(),
        });
    }

    let ty = if is_string_name {
        TokenType::StringName
    } else {
        TokenType::String
    };
    let value = if is_string_name {
        Variant::StringName(s.clone())
    } else {
        Variant::String(s.clone())
    };
    Ok(Token { ty, value, raw: s })
}

/// 解析标识符或数字。移植自 Godot `get_token` 的 default 分支。
fn parse_identifier_or_number(stream: &mut Stream, first_char: char) -> Result<Token, LexError> {
    let mut text = String::new();
    let mut c = first_char;

    if c == '-' {
        text.push('-');
        c = stream.get_char();
    }

    if c.is_ascii_digit() {
        // 数字：状态机解析
        return parse_number(stream, text, c);
    }

    // 标识符：字母/下划线开头，后跟字母数字下划线
    if is_ident_start(c) {
        text.push(c);
        loop {
            let next = stream.get_char();
            if is_ident_part(next) {
                text.push(next);
            } else {
                stream.saved = next;
                break;
            }
        }
        return Ok(Token {
            ty: TokenType::Identifier,
            value: Variant::Null, // 标识符的语义在 parse_value 阶段解析
            raw: text,
        });
    }

    Err(LexError {
        line: stream.line,
        message: format!("Unexpected character '{}'", c),
    })
}

/// 数字解析状态机。移植自 Godot `get_token` 的数字分支。
fn parse_number(stream: &mut Stream, mut text: String, first_digit: char) -> Result<Token, LexError> {
    const READING_INT: u8 = 1;
    const READING_DEC: u8 = 2;
    const READING_EXP: u8 = 3;
    const READING_DONE: u8 = 4;

    let mut reading = READING_INT;
    let mut c = first_digit;
    let mut exp_sign = false;
    let mut exp_beg = false;
    let mut is_float = false;

    loop {
        match reading {
            READING_INT => {
                if c.is_ascii_digit() {
                    text.push(c);
                } else if c == '.' {
                    reading = READING_DEC;
                    is_float = true;
                    text.push(c);
                } else if c == 'e' || c == 'E' {
                    reading = READING_EXP;
                    is_float = true;
                    text.push(c);
                } else {
                    reading = READING_DONE;
                }
            }
            READING_DEC => {
                if c.is_ascii_digit() {
                    text.push(c);
                } else if c == 'e' || c == 'E' {
                    reading = READING_EXP;
                    is_float = true;
                    text.push(c);
                } else {
                    reading = READING_DONE;
                }
            }
            READING_EXP => {
                if c.is_ascii_digit() {
                    if !exp_beg {
                        exp_beg = true;
                    }
                    text.push(c);
                } else if (c == '-' || c == '+') && !exp_sign && !exp_beg {
                    exp_sign = true;
                    text.push(c);
                } else {
                    reading = READING_DONE;
                }
            }
            _ => {}
        }
        if reading == READING_DONE {
            stream.saved = c;
            break;
        }
        c = stream.get_char();
        if c == '\0' && stream.is_eof() {
            break;
        }
    }

    let value = if is_float {
        match text.parse::<f64>() {
            Ok(f) => Variant::Float(f as f32),
            Err(_) => {
                return Err(LexError {
                    line: stream.line,
                    message: format!("Invalid number: {}", text),
                })
            }
        }
    } else {
        match text.parse::<i64>() {
            Ok(i) => Variant::Int(i),
            Err(_) => {
                return Err(LexError {
                    line: stream.line,
                    message: format!("Invalid number: {}", text),
                })
            }
        }
    };
    Ok(Token {
        ty: TokenType::Number,
        value,
        raw: text,
    })
}

/// 解析 `#RRGGBBAA` / `#RGB` / `#RGBA` 形式的 Color。移植自 Godot `Color::html`。
fn parse_color_hex(s: &str) -> Variant {
    // s 形如 "#FFFFFF" / "#RRGGBBAA" / "#RGB" / "#RGBA"
    let hex = &s[1..]; // 去掉 #
    let parse = |start: usize, len: usize| -> f32 {
        if start + len > hex.len() {
            return 1.0;
        }
        u8::from_str_radix(&hex[start..start + len], 16).map(|v| v as f32 / 255.0).unwrap_or(1.0)
    };
    let (r, g, b, a) = match hex.len() {
        3 => {
            // #RGB
            (
                u8::from_str_radix(&hex[0..1].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                u8::from_str_radix(&hex[1..2].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                u8::from_str_radix(&hex[2..3].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                1.0,
            )
        }
        4 => {
            // #RGBA
            (
                u8::from_str_radix(&hex[0..1].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                u8::from_str_radix(&hex[1..2].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                u8::from_str_radix(&hex[2..3].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(0.0),
                u8::from_str_radix(&hex[3..4].repeat(2), 16).map(|v| v as f32 / 255.0).unwrap_or(1.0),
            )
        }
        6 => (parse(0, 2), parse(2, 2), parse(4, 2), 1.0), // #RRGGBB
        8 => (parse(0, 2), parse(2, 2), parse(4, 2), parse(6, 2)), // #RRGGBBAA
        _ => (0.0, 0.0, 0.0, 1.0),
    };
    Variant::Color(Vec4::new(r, g, b, a))
}

fn is_hex_digit(c: char) -> bool {
    c.is_ascii_digit() || ('a'..='f').contains(&c) || ('A'..='F').contains(&c)
}

fn hex_value(c: char) -> u32 {
    if c.is_ascii_digit() {
        (c as u32) - ('0' as u32)
    } else if ('a'..='f').contains(&c) {
        (c as u32) - ('a' as u32) + 10
    } else if ('A'..='F').contains(&c) {
        (c as u32) - ('A' as u32) + 10
    } else {
        0
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_all(input: &str) -> Vec<Token> {
        let mut s = Stream::new(input);
        let mut out = Vec::new();
        loop {
            let t = get_token(&mut s).expect("lex 应成功");
            if t.ty == TokenType::Eof {
                break;
            }
            out.push(t);
        }
        out
    }

    #[test]
    fn lex_punctuation() {
        let tokens = lex_all("{ } [ ] ( ) : , . =");
        let types: Vec<_> = tokens.iter().map(|t| t.ty.clone()).collect();
        assert_eq!(
            types,
            vec![
                TokenType::CurlyOpen,
                TokenType::CurlyClose,
                TokenType::BracketOpen,
                TokenType::BracketClose,
                TokenType::ParenOpen,
                TokenType::ParenClose,
                TokenType::Colon,
                TokenType::Comma,
                TokenType::Period,
                TokenType::Equal,
            ]
        );
    }

    #[test]
    fn lex_numbers() {
        let tokens = lex_all("42 -7 3.14 1e3 -0.5 1E-3");
        assert_eq!(tokens[0].value, Variant::Int(42));
        assert_eq!(tokens[1].value, Variant::Int(-7));
        assert_eq!(tokens[2].value, Variant::Float(3.14));
        assert_eq!(tokens[3].value, Variant::Float(1000.0));
        assert_eq!(tokens[4].value, Variant::Float(-0.5));
        assert_eq!(tokens[5].value, Variant::Float(0.001));
    }

    #[test]
    fn lex_string_and_stringname() {
        let tokens = lex_all("\"hi\" &\"name\"");
        assert_eq!(tokens[0].ty, TokenType::String);
        assert_eq!(tokens[0].value, Variant::String("hi".into()));
        assert_eq!(tokens[1].ty, TokenType::StringName);
        assert_eq!(tokens[1].value, Variant::StringName("name".into()));
    }

    #[test]
    fn lex_string_with_escapes() {
        let tokens = lex_all("\"a\\nb\\tc\\\\d\"");
        assert_eq!(tokens[0].value, Variant::String("a\nb\tc\\d".into()));
    }

    #[test]
    fn lex_color_hex() {
        let tokens = lex_all("#FF0000FF #FFF");
        assert_eq!(tokens[0].ty, TokenType::Color);
        // #FF0000FF = 红色不透明
        if let Variant::Color(c) = tokens[0].value {
            assert!((c.x - 1.0).abs() < 1e-6); // R
            assert!((c.y - 0.0).abs() < 1e-6); // G
            assert!((c.z - 0.0).abs() < 1e-6); // B
            assert!((c.w - 1.0).abs() < 1e-6); // A
        } else {
            panic!("应该是 Color");
        }
    }

    #[test]
    fn lex_identifier() {
        let tokens = lex_all("Vector2 null inf");
        assert_eq!(tokens[0].ty, TokenType::Identifier);
        assert_eq!(tokens[0].raw, "Vector2");
        assert_eq!(tokens[1].raw, "null");
        assert_eq!(tokens[2].raw, "inf");
    }
}
