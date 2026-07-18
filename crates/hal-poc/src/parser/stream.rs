//! 字符流，对应 Godot `VariantParser::Stream`。
//!
//! 移植自 `core/variant/variant_parser.cpp` 的 Stream/StreamString。
//! 提供 `get_char()` / `is_eof()` / `saved`（一个字符的回退缓冲）。

/// 字符流。包装字符串输入，提供逐字符读取和一个字符的回退。
pub struct Stream<'a> {
    chars: Vec<char>,
    pos: usize,
    /// 回退缓冲（对应 Godot Stream::saved）。0 表示无回退。
    pub saved: char,
    /// 是否已读到末尾。
    eof: bool,
    /// 当前行号（从 1 开始，对应 Godot 的 line）。
    pub line: usize,
    _phantom: std::marker::PhantomData<&'a str>,
}

impl<'a> Stream<'a> {
    /// 从字符串构造字符流。
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            saved: '\0',
            eof: false,
            line: 1,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 读一个字符。优先返回 saved。
    /// 遇到 `\n` 时增加 line 计数。
    pub fn get_char(&mut self) -> char {
        if self.saved != '\0' {
            let c = self.saved;
            self.saved = '\0';
            if c == '\n' {
                self.line += 1;
            }
            return c;
        }
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
            }
            c
        } else {
            self.eof = true;
            '\0'
        }
    }

    /// 是否读到末尾。
    pub fn is_eof(&self) -> bool {
        self.eof || (self.saved == '\0' && self.pos >= self.chars.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_reads_chars_and_tracks_lines() {
        let mut s = Stream::new("ab\ncd");
        assert_eq!(s.line, 1);
        assert_eq!(s.get_char(), 'a');
        assert_eq!(s.get_char(), 'b');
        assert_eq!(s.get_char(), '\n');
        assert_eq!(s.line, 2);
        assert_eq!(s.get_char(), 'c');
        assert_eq!(s.get_char(), 'd');
        assert_eq!(s.get_char(), '\0');
        assert!(s.is_eof());
    }

    #[test]
    fn stream_unget_via_saved() {
        let mut s = Stream::new("xyz");
        let c = s.get_char();
        assert_eq!(c, 'x');
        s.saved = c; // 回退
        assert_eq!(s.get_char(), 'x'); // 再次读出
        assert_eq!(s.get_char(), 'y');
    }
}
