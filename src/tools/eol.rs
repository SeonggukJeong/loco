//! 라인엔딩 정책 (스펙 §4): 매칭·비교는 \n 정규화, 쓰기 시 지배적 EOL 복원

pub fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// CRLF가 lone LF보다 많으면 true (덮어쓰기 시 CRLF 유지 판단)
pub fn dominant_crlf(s: &str) -> bool {
    let crlf = s.matches("\r\n").count();
    let lf = s.matches('\n').count() - crlf;
    crlf > lf
}

pub fn restore_eol(s: &str, crlf: bool) -> String {
    if crlf { s.replace('\n', "\r\n") } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_restore_roundtrip() {
        assert_eq!(normalize_eol("a\r\nb\nc"), "a\nb\nc");
        assert_eq!(restore_eol("a\nb", true), "a\r\nb");
        assert_eq!(restore_eol("a\nb", false), "a\nb");
    }

    #[test]
    fn dominant_crlf_counts_majority() {
        assert!(dominant_crlf("a\r\nb\r\nc\n"));
        assert!(!dominant_crlf("a\nb\nc\r\n"));
        assert!(!dominant_crlf("no newline"));
    }
}
