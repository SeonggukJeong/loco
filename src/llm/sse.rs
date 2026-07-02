/// Server-Sent Events 라인 파서. 바이트를 누적하다 완성된 줄에서
/// `data:` 페이로드를 추출한다. 청크가 줄/UTF-8 문자 중간에서 잘려도 안전.
#[derive(Default)]
pub struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(data) = line.strip_prefix("data:") {
                out.push(data.trim_start().to_string());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_data_lines() {
        let mut p = SseParser::new();
        let out = p.feed(b"data: {\"a\":1}\n\ndata: [DONE]\n\n");
        assert_eq!(out, vec![r#"{"a":1}"#.to_string(), "[DONE]".to_string()]);
    }

    #[test]
    fn handles_chunk_split_mid_line() {
        let mut p = SseParser::new();
        assert!(p.feed(b"data: {\"content\":").is_empty());
        let out = p.feed(b"\"hi\"}\n");
        assert_eq!(out, vec![r#"{"content":"hi"}"#.to_string()]);
    }

    #[test]
    fn handles_chunk_split_mid_utf8_char() {
        let mut p = SseParser::new();
        let full = "data: {\"c\":\"안녕\"}\n".as_bytes();
        // 멀티바이트 문자 중간에서 자름
        let cut = full.len() - 4;
        assert!(p.feed(&full[..cut]).is_empty());
        let out = p.feed(&full[cut..]);
        assert_eq!(out, vec![r#"{"c":"안녕"}"#.to_string()]);
    }

    #[test]
    fn ignores_non_data_lines_and_crlf() {
        let mut p = SseParser::new();
        let out = p.feed(b"event: ping\r\ndata: x\r\n\r\n: comment\n");
        assert_eq!(out, vec!["x".to_string()]);
    }
}
