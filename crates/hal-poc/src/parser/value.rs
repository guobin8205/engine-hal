//! Variant 值解析，对应 Godot `VariantParser::parse_value`。
//!
//! 移植自 `core/variant/variant_parser.cpp:677-1642`。
//! 按 token 类型递归下降，构造 Variant。

use glam::{Vec2, Vec3, Vec4};

use crate::parser::lexer::{get_token, LexError, Token, TokenType};
use crate::parser::stream::Stream;
use crate::types::Variant;

/// 解析错误：附行列信息。
#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError {
            line: e.line,
            message: e.message,
        }
    }
}

fn err(line: usize, msg: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: msg.into(),
    }
}

/// 解析一个 Variant 值。给定已经读出的首 token，递归解析剩余部分。
///
/// 对应 Godot `parse_value`。
pub fn parse_value(token: Token, stream: &mut Stream) -> Result<Variant, ParseError> {
    match token.ty {
        TokenType::CurlyOpen => parse_dictionary(stream),
        TokenType::BracketOpen => parse_array(stream),
        TokenType::Number => Ok(token.value),
        TokenType::String => Ok(token.value),
        TokenType::StringName => Ok(token.value),
        TokenType::Color => Ok(token.value),
        TokenType::Identifier => parse_identifier_value(&token.raw, stream),
        _ => Err(err(stream.line, format!("Unexpected token: {:?}", token.ty))),
    }
}

/// 处理标识符值。对应 Godot parse_value 里 `id == ...` 长链。
fn parse_identifier_value(id: &str, stream: &mut Stream) -> Result<Variant, ParseError> {
    // 标量字面量
    match id {
        "true" => return Ok(Variant::Bool(true)),
        "false" => return Ok(Variant::Bool(false)),
        "null" | "nil" => return Ok(Variant::Null),
        "inf" => return Ok(Variant::Float(f32::INFINITY)),
        "-inf" | "inf_neg" => return Ok(Variant::Float(f32::NEG_INFINITY)),
        "nan" => return Ok(Variant::Float(f32::NAN)),
        _ => {}
    }

    // 资源引用（ExtResource/SubResource）
    if id == "ExtResource" || id == "SubResource" {
        let id_str = parse_single_string_arg(stream)?;
        return Ok(if id == "ExtResource" {
            Variant::ExtResource(id_str)
        } else {
            Variant::SubResource(id_str)
        });
    }

    // NodePath("...")
    if id == "NodePath" {
        let s = parse_single_string_arg(stream)?;
        return Ok(Variant::NodePath(s));
    }

    // RID() / RID(id)
    if id == "RID" {
        let args = parse_construct_int(stream)?;
        return Ok(Variant::RID(args.first().copied().unwrap_or(0) as u64));
    }

    // Signal() / Callable() — 永远空载入
    if id == "Signal" || id == "Callable" {
        skip_empty_parens(stream)?;
        return Ok(Variant::Null);
    }

    // 几何类型构造器
    if let Some(v) = try_parse_geometric(id, stream)? {
        return Ok(v);
    }

    // Packed 数组（typed constructor 形式）
    if let Some(v) = try_parse_packed(id, stream)? {
        return Ok(v);
    }

    // Array / Dictionary（typed collection）
    if id == "Array" || id == "Dictionary" {
        return parse_typed_collection(id, stream);
    }

    // 未知构造器：容错降级，保留 type_name 和参数
    let args = parse_construct_value_list(stream)?;
    Ok(Variant::UnknownConstructor {
        type_name: id.to_string(),
        args,
    })
}

/// 尝试解析几何类型。返回 None 表示不是几何类型，让调用方继续尝试。
fn try_parse_geometric(id: &str, stream: &mut Stream) -> Result<Option<Variant>, ParseError> {
    macro_rules! real_typed {
        ($name:expr, $count:expr, $variant:ident) => {{
            if id == $name {
                let args = parse_construct_real(stream)?;
                if args.len() != $count {
                    return Err(err(
                        stream.line,
                        format!("Expected {} args for {}", $count, $name),
                    ));
                }
                let mut arr = [0.0f32; $count];
                for (i, v) in args.iter().enumerate() {
                    arr[i] = *v;
                }
                return Ok(Some(Variant::$variant(arr)));
            }
        }};
    }

    if id == "Vector2" {
        let a = parse_construct_real(stream)?;
        if a.len() != 2 {
            return Err(err(stream.line, "Expected 2 args for Vector2"));
        }
        return Ok(Some(Variant::Vector2(Vec2::new(a[0], a[1]))));
    }
    if id == "Vector2i" {
        let a = parse_construct_int(stream)?;
        if a.len() != 2 {
            return Err(err(stream.line, "Expected 2 args for Vector2i"));
        }
        return Ok(Some(Variant::Vector2i([a[0], a[1]])));
    }
    if id == "Vector3" {
        let a = parse_construct_real(stream)?;
        if a.len() != 3 {
            return Err(err(stream.line, "Expected 3 args for Vector3"));
        }
        return Ok(Some(Variant::Vector3(Vec3::new(a[0], a[1], a[2]))));
    }
    if id == "Vector3i" {
        let a = parse_construct_int(stream)?;
        if a.len() != 3 {
            return Err(err(stream.line, "Expected 3 args for Vector3i"));
        }
        return Ok(Some(Variant::Vector3i([a[0], a[1], a[2]])));
    }
    if id == "Vector4" {
        let a = parse_construct_real(stream)?;
        if a.len() != 4 {
            return Err(err(stream.line, "Expected 4 args for Vector4"));
        }
        return Ok(Some(Variant::Vector4(Vec4::new(a[0], a[1], a[2], a[3]))));
    }
    if id == "Vector4i" {
        let a = parse_construct_int(stream)?;
        if a.len() != 4 {
            return Err(err(stream.line, "Expected 4 args for Vector4i"));
        }
        return Ok(Some(Variant::Vector4i([a[0], a[1], a[2], a[3]])));
    }
    if id == "Color" {
        let a = parse_construct_real(stream)?;
        if a.len() != 3 && a.len() != 4 {
            return Err(err(stream.line, "Expected 3 or 4 args for Color"));
        }
        let r = a[0];
        let g = a[1];
        let b = a[2];
        let alpha = a.get(3).copied().unwrap_or(1.0);
        return Ok(Some(Variant::Color(Vec4::new(r, g, b, alpha))));
    }

    // 固定长度 real 数组类型
    real_typed!("Rect2", 4, Rect2);
    real_typed!("Plane", 4, Plane);
    real_typed!("Quaternion", 4, Quaternion);
    real_typed!("Quat", 4, Quaternion); // 兼容
    real_typed!("Transform2D", 6, Transform2D);
    real_typed!("Matrix32", 6, Transform2D); // 兼容
    real_typed!("AABB", 6, AABB);
    real_typed!("Rect3", 6, AABB); // 兼容
    real_typed!("Basis", 9, Basis);
    real_typed!("Matrix3", 9, Basis); // 兼容
    real_typed!("Transform3D", 12, Transform3D);
    real_typed!("Transform", 12, Transform3D); // 兼容
    real_typed!("Projection", 16, Projection);

    // Rect2i 用 int
    if id == "Rect2i" {
        let a = parse_construct_int(stream)?;
        if a.len() != 4 {
            return Err(err(stream.line, "Expected 4 args for Rect2i"));
        }
        return Ok(Some(Variant::Rect2i([a[0], a[1], a[2], a[3]])));
    }

    Ok(None)
}

/// 尝试解析 Packed* 数组类型。
fn try_parse_packed(id: &str, stream: &mut Stream) -> Result<Option<Variant>, ParseError> {
    // 注意：Packed* 参数按 real/int/string 序列收集，Godot 不检查类型严格性
    let result: Option<Variant> = match id {
        "PackedByteArray" | "ByteArray" | "PoolByteArray" => {
            // PackedByteArray 可能是数字列表或 base64 字符串
            Some(parse_packed_byte_array(stream)?)
        }
        "PackedInt32Array" | "PackedIntArray" | "PoolIntArray" | "IntArray" => {
            let ints = parse_construct_int(stream)?;
            Some(Variant::PackedInt32Array(ints.into_iter().map(|i| i as i32).collect()))
        }
        "PackedInt64Array" => {
            let ints = parse_construct_int(stream)?;
            Some(Variant::PackedInt64Array(ints))
        }
        "PackedFloat32Array" | "PackedRealArray" | "PoolRealArray" | "FloatArray" => {
            let reals = parse_construct_real(stream)?;
            Some(Variant::PackedFloat32Array(reals))
        }
        "PackedFloat64Array" => {
            let reals = parse_construct_real(stream)?;
            Some(Variant::PackedFloat64Array(reals.into_iter().map(|f| f as f64).collect()))
        }
        "PackedStringArray" | "PoolStringArray" | "StringArray" => {
            let strs = parse_construct_string(stream)?;
            Some(Variant::PackedStringArray(strs))
        }
        "PackedVector2Array" | "PoolVector2Array" | "Vector2Array" => {
            // real 列表，每 2 个一组
            let reals = parse_construct_real(stream)?;
            let vecs: Vec<Vec2> = reals
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| Vec2::new(c[0], c[1]))
                .collect();
            Some(Variant::PackedVector2Array(vecs))
        }
        "PackedVector3Array" | "PoolVector3Array" | "Vector3Array" => {
            let reals = parse_construct_real(stream)?;
            let vecs: Vec<Vec3> = reals
                .chunks(3)
                .filter(|c| c.len() == 3)
                .map(|c| Vec3::new(c[0], c[1], c[2]))
                .collect();
            Some(Variant::PackedVector3Array(vecs))
        }
        "PackedVector4Array" | "PoolVector4Array" | "Vector4Array" => {
            let reals = parse_construct_real(stream)?;
            let vecs: Vec<Vec4> = reals
                .chunks(4)
                .filter(|c| c.len() == 4)
                .map(|c| Vec4::new(c[0], c[1], c[2], c[3]))
                .collect();
            Some(Variant::PackedVector4Array(vecs))
        }
        "PackedColorArray" | "PoolColorArray" | "ColorArray" => {
            let reals = parse_construct_real(stream)?;
            let colors: Vec<Vec4> = reals
                .chunks(4)
                .filter(|c| c.len() == 4)
                .map(|c| Vec4::new(c[0], c[1], c[2], c[3]))
                .collect();
            Some(Variant::PackedColorArray(colors))
        }
        _ => return Ok(None),
    };
    Ok(result)
}

/// 处理 typed collection `Array[T]([...])` / `Dictionary[K,V]({...})`。
/// 也处理无类型 `Array([...])` / `Dictionary({...})`（少见，但源码支持）。
fn parse_typed_collection(id: &str, stream: &mut Stream) -> Result<Variant, ParseError> {
    let mut next = get_token(stream)?;
    // 可选的类型参数：[Type] 或 [K, V]
    let mut type_args: Vec<String> = Vec::new();
    if next.ty == TokenType::BracketOpen {
        // 读类型参数直到 ]
        loop {
            let t = get_token(stream)?;
            if t.ty == TokenType::BracketClose {
                break;
            }
            if t.ty == TokenType::Identifier || t.ty == TokenType::String {
                type_args.push(t.raw);
            } else if t.ty == TokenType::Comma {
                // 继续
            } else {
                return Err(err(stream.line, "Expected type in Array[...]"));
            }
        }
        next = get_token(stream)?;
    }

    if next.ty != TokenType::ParenOpen {
        return Err(err(stream.line, format!("Expected '(' after {}", id)));
    }
    // 内部是 [items] 或 {entries}
    let inner = get_token(stream)?;
    let items: Vec<Variant> = match inner.ty {
        TokenType::BracketOpen => parse_array_contents(stream)?,
        TokenType::CurlyOpen => {
            // Dictionary({...}) — 收集为 entries
            return finish_typed_dict(type_args, stream);
        }
        _ => return Err(err(stream.line, "Expected '[' or '{' inside collection")),
    };
    // 闭合 )
    let close = get_token(stream)?;
    if close.ty != TokenType::ParenClose {
        return Err(err(stream.line, "Expected ')' to close collection"));
    }
    if id == "Dictionary" {
        // 不应该走到这里（dict 应走 CurlyOpen 分支），保险起见
        return Ok(Variant::Dict(items.into_iter().map(|v| (Variant::Null, v)).collect()));
    }
    if type_args.is_empty() {
        Ok(Variant::Array(items))
    } else {
        Ok(Variant::TypedArray {
            elem_type: type_args.join(","),
            items,
        })
    }
}

fn finish_typed_dict(
    mut type_args: Vec<String>,
    stream: &mut Stream,
) -> Result<Variant, ParseError> {
    let entries = parse_dictionary_contents(stream)?;
    let close = get_token(stream)?;
    if close.ty != TokenType::ParenClose {
        return Err(err(stream.line, "Expected ')' to close Dictionary(...)"));
    }
    if type_args.is_empty() {
        Ok(Variant::Dict(entries))
    } else {
        let value_type = type_args.pop().unwrap_or_default();
        let key_type = type_args.pop().unwrap_or_default();
        Ok(Variant::TypedDict {
            key_type,
            value_type,
            entries,
        })
    }
}

/// 解析 `[...]` 内部内容（已读 `[`）。对应 Godot `_parse_array`。
fn parse_array_contents(stream: &mut Stream) -> Result<Vec<Variant>, ParseError> {
    let mut array = Vec::new();
    let mut need_comma = false;
    loop {
        if stream.is_eof() {
            return Err(err(stream.line, "Unexpected EOF in array"));
        }
        let token = get_token(stream)?;
        if token.ty == TokenType::BracketClose {
            return Ok(array);
        }
        if need_comma {
            if token.ty != TokenType::Comma {
                return Err(err(stream.line, "Expected ',' in array"));
            }
            need_comma = false;
            continue;
        }
        let v = parse_value(token, stream)?;
        array.push(v);
        need_comma = true;
    }
}

/// 解析 `{...}` 内部内容（已读 `{`）。对应 Godot `_parse_dictionary`。
fn parse_dictionary_contents(stream: &mut Stream) -> Result<Vec<(Variant, Variant)>, ParseError> {
    let mut entries = Vec::new();
    let mut at_key = true;
    let mut need_comma = false;
    let mut key = Variant::Null;

    loop {
        if stream.is_eof() {
            return Err(err(stream.line, "Unexpected EOF in dictionary"));
        }
        if at_key {
            let token = get_token(stream)?;
            if token.ty == TokenType::CurlyClose {
                return Ok(entries);
            }
            if need_comma {
                if token.ty != TokenType::Comma {
                    return Err(err(stream.line, "Expected '}' or ',' in dict"));
                }
                need_comma = false;
                continue;
            }
            key = parse_value(token, stream)?;
            let colon = get_token(stream)?;
            if colon.ty != TokenType::Colon {
                return Err(err(stream.line, "Expected ':' in dict"));
            }
            at_key = false;
        } else {
            let token = get_token(stream)?;
            let v = parse_value(token, stream)?;
            entries.push((key.clone(), v));
            need_comma = true;
            at_key = true;
        }
    }
}

/// 解析 `{` 开始的字典（顶层入口）。
fn parse_dictionary(stream: &mut Stream) -> Result<Variant, ParseError> {
    let entries = parse_dictionary_contents(stream)?;
    Ok(Variant::Dict(entries))
}

/// 解析 `[` 开始的数组（顶层入口）。
fn parse_array(stream: &mut Stream) -> Result<Variant, ParseError> {
    let items = parse_array_contents(stream)?;
    Ok(Variant::Array(items))
}

/// 解析 `(num, num, ...)` 形式的 real 构造器参数。对应 Godot `_parse_construct<real_t>`。
fn parse_construct_real(stream: &mut Stream) -> Result<Vec<f32>, ParseError> {
    let mut out = Vec::new();
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '(' in constructor"));
    }
    let mut first = true;
    loop {
        if !first {
            let t = get_token(stream)?;
            match t.ty {
                TokenType::Comma => {}
                TokenType::ParenClose => break,
                _ => return Err(err(stream.line, "Expected ',' or ')' in constructor")),
            }
        }
        let t = get_token(stream)?;
        if first && t.ty == TokenType::ParenClose {
            break;
        }
        let v = token_to_real(&t, stream)?;
        out.push(v);
        first = false;
    }
    Ok(out)
}

/// 解析 `(num, num, ...)` 形式的 int 构造器参数。
fn parse_construct_int(stream: &mut Stream) -> Result<Vec<i64>, ParseError> {
    let mut out = Vec::new();
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '(' in constructor"));
    }
    let mut first = true;
    loop {
        if !first {
            let t = get_token(stream)?;
            match t.ty {
                TokenType::Comma => {}
                TokenType::ParenClose => break,
                _ => return Err(err(stream.line, "Expected ',' or ')' in constructor")),
            }
        }
        let t = get_token(stream)?;
        if first && t.ty == TokenType::ParenClose {
            break;
        }
        let v = token_to_int(&t, stream)?;
        out.push(v);
        first = false;
    }
    Ok(out)
}

/// 解析 `("a", "b", ...)` 形式的字符串参数列表。
fn parse_construct_string(stream: &mut Stream) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '(' in constructor"));
    }
    let mut first = true;
    loop {
        if !first {
            let t = get_token(stream)?;
            match t.ty {
                TokenType::Comma => {}
                TokenType::ParenClose => break,
                _ => return Err(err(stream.line, "Expected ',' or ')' in constructor")),
            }
        }
        let t = get_token(stream)?;
        if first && t.ty == TokenType::ParenClose {
            break;
        }
        match t.ty {
            TokenType::String | TokenType::StringName => {
                if let Variant::String(s) | Variant::StringName(s) = t.value {
                    out.push(s);
                }
            }
            _ => return Err(err(stream.line, "Expected string in constructor")),
        }
        first = false;
    }
    Ok(out)
}

/// 解析 `(value, value, ...)` 形式的 Variant 列表（用于未知构造器降级）。
fn parse_construct_value_list(stream: &mut Stream) -> Result<Vec<Variant>, ParseError> {
    let mut out = Vec::new();
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '(' in constructor"));
    }
    let mut first = true;
    loop {
        if !first {
            let t = get_token(stream)?;
            match t.ty {
                TokenType::Comma => {}
                TokenType::ParenClose => break,
                _ => return Err(err(stream.line, "Expected ',' or ')' in constructor")),
            }
        }
        let t = get_token(stream)?;
        if first && t.ty == TokenType::ParenClose {
            break;
        }
        let v = parse_value(t, stream)?;
        out.push(v);
        first = false;
    }
    Ok(out)
}

/// 解析单个字符串参数：`("...")`。用于 ExtResource/SubResource/NodePath。
fn parse_single_string_arg(stream: &mut Stream) -> Result<String, ParseError> {
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '('"));
    }
    let s = get_token(stream)?;
    let result = match s.value {
        Variant::String(s) | Variant::StringName(s) => s,
        _ => return Err(err(stream.line, "Expected string argument")),
    };
    let close = get_token(stream)?;
    if close.ty != TokenType::ParenClose {
        return Err(err(stream.line, "Expected ')'"));
    }
    Ok(result)
}

/// 跳过空括号 `()`。
fn skip_empty_parens(stream: &mut Stream) -> Result<(), ParseError> {
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '('"));
    }
    let close = get_token(stream)?;
    if close.ty != TokenType::ParenClose {
        return Err(err(stream.line, "Expected ')'"));
    }
    Ok(())
}

/// 解析 PackedByteArray：可能是数字列表，或 base64 字符串。
fn parse_packed_byte_array(stream: &mut Stream) -> Result<Variant, ParseError> {
    let open = get_token(stream)?;
    if open.ty != TokenType::ParenOpen {
        return Err(err(stream.line, "Expected '('"));
    }
    let first = get_token(stream)?;
    match first.ty {
        TokenType::String => {
            // base64 字符串
            let _b64 = if let Variant::String(s) = first.value {
                s
            } else {
                String::new()
            };
            // POC-A 不实际解码 base64，保留为空数组（标注 TODO）
            let close = get_token(stream)?;
            if close.ty != TokenType::ParenClose {
                return Err(err(stream.line, "Expected ')'"));
            }
            Ok(Variant::PackedByteArray(Vec::new()))
        }
        TokenType::Number | TokenType::Identifier => {
            // 数字列表（first 已经读了第一个）
            let mut bytes = Vec::new();
            let v = token_to_int(&first, stream)?;
            bytes.push(v as u8);
            loop {
                let t = get_token(stream)?;
                match t.ty {
                    TokenType::Comma => {
                        let nt = get_token(stream)?;
                        let v = token_to_int(&nt, stream)?;
                        bytes.push(v as u8);
                    }
                    TokenType::ParenClose => break,
                    _ => return Err(err(stream.line, "Expected ',' or ')' in PackedByteArray")),
                }
            }
            Ok(Variant::PackedByteArray(bytes))
        }
        TokenType::ParenClose => Ok(Variant::PackedByteArray(Vec::new())),
        _ => Err(err(stream.line, "Expected base64 string or numbers in PackedByteArray")),
    }
}

/// 把 token 转 real，处理 inf/-inf/nan 标识符。对应 Godot `stor_fix`。
fn token_to_real(t: &Token, _stream: &Stream) -> Result<f32, ParseError> {
    match t.ty {
        TokenType::Number => match t.value {
            Variant::Float(f) => Ok(f),
            Variant::Int(i) => Ok(i as f32),
            _ => Err(err(_stream.line, "Expected number")),
        },
        TokenType::Identifier => match t.raw.as_str() {
            "inf" => Ok(f32::INFINITY),
            "-inf" | "inf_neg" => Ok(f32::NEG_INFINITY),
            "nan" => Ok(f32::NAN),
            _ => Err(err(_stream.line, format!("Expected number, got '{}'", t.raw))),
        },
        _ => Err(err(_stream.line, "Expected number")),
    }
}

/// 把 token 转 int。
fn token_to_int(t: &Token, stream: &Stream) -> Result<i64, ParseError> {
    match t.ty {
        TokenType::Number => match t.value {
            Variant::Int(i) => Ok(i),
            Variant::Float(f) => Ok(f as i64),
            _ => Err(err(stream.line, "Expected integer")),
        },
        _ => Err(err(stream.line, "Expected integer")),
    }
}
