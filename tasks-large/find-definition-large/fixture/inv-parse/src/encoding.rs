//! 인코딩 관련 사전 점검.
//!
//! 이 크레이트는 항상 UTF-8로 디코딩된 `&str`을 입력받는다(바이트 단계의
//! 실제 디코딩은 상위 계층이 담당). 다만 원본이 CP949(EUC-KR 계열)로
//! 인코딩된 파일을 UTF-8로 잘못 강제 디코딩하면 한글이 깨져 대체 문자로
//! 뭉개지는 경우가 흔해, 그 흔적을 감지하는 휴리스틱을 여기 둔다.

/// 유니코드 치환 문자(U+FFFD)의 개수를 센다. 이 문자가 여러 개 나오면
/// 잘못된 인코딩으로 디코딩되었을 가능성이 높다.
pub fn count_replacement_chars(text: &str) -> usize {
    text.chars().filter(|&c| c == '\u{fffd}').count()
}

/// 텍스트에 인코딩 손상 흔적(치환 문자)이 있는지 검사한다.
pub fn looks_mangled(text: &str) -> bool {
    count_replacement_chars(text) > 0
}

/// UTF-8 BOM 바이트 시퀀스.
pub const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// 바이트열이 UTF-8 BOM으로 시작하는지 검사한다.
pub fn has_utf8_bom(bytes: &[u8]) -> bool {
    bytes.starts_with(&UTF8_BOM)
}

/// 텍스트에 CRLF 줄바꿈이 하나라도 있는지 검사한다.
pub fn has_crlf(text: &str) -> bool {
    text.contains("\r\n")
}

/// 텍스트가 CRLF와 단독 LF를 섞어 쓰고 있는지(줄바꿈 스타일이 혼재하는지)
/// 검사한다 — 여러 시스템을 거쳐온 배치 파일에서 흔히 나타난다.
pub fn has_mixed_line_endings(text: &str) -> bool {
    let has_crlf = text.contains("\r\n");
    let has_lone_lf = text
        .as_bytes()
        .windows(2)
        .enumerate()
        .any(|(i, w)| w[1] == b'\n' && w[0] != b'\r' && i > 0)
        || (text.starts_with('\n'));
    has_crlf && has_lone_lf
}

/// 순수 ASCII 문자만으로 이루어졌는지 검사한다(한글 등 비ASCII가 전혀
/// 없으면 인코딩 문제를 걱정할 필요가 없다는 뜻이라 사전 필터로 쓴다).
pub fn is_ascii_only(text: &str) -> bool {
    text.is_ascii()
}

/// 비ASCII 문자 비율(%)을 계산한다.
pub fn non_ascii_ratio_percent(text: &str) -> u32 {
    let total = text.chars().count();
    if total == 0 {
        return 0;
    }
    let non_ascii = text.chars().filter(|c| !c.is_ascii()).count();
    (non_ascii * 100 / total) as u32
}

/// 텍스트가 제어 문자(탭/개행 제외)를 포함하는지 검사한다 — 바이너리
/// 파일을 텍스트로 잘못 읽은 징후일 수 있다.
pub fn has_suspicious_control_chars(text: &str) -> bool {
    text.chars().any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r')
}

/// 인코딩 관련 점검 결과 요약.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EncodingReport {
    pub has_bom: bool,
    pub mangled: bool,
    pub mixed_line_endings: bool,
    pub has_control_chars: bool,
}

/// 텍스트/원본 바이트에 대해 인코딩 점검을 한 번에 실행한다.
pub fn inspect(bytes: &[u8], text: &str) -> EncodingReport {
    EncodingReport {
        has_bom: has_utf8_bom(bytes),
        mangled: looks_mangled(text),
        mixed_line_endings: has_mixed_line_endings(text),
        has_control_chars: has_suspicious_control_chars(text),
    }
}

impl EncodingReport {
    /// 배치를 그대로 처리해도 안전한지(문제 신호가 하나도 없는지) 판정한다.
    pub fn is_clean(&self) -> bool {
        !self.mangled && !self.has_control_chars
    }

    /// 점검 결과를 사람이 읽는 한 줄 요약으로 만든다.
    pub fn summary_line(&self) -> String {
        let mut flags = Vec::new();
        if self.has_bom {
            flags.push("BOM");
        }
        if self.mangled {
            flags.push("인코딩깨짐의심");
        }
        if self.mixed_line_endings {
            flags.push("줄바꿈혼재");
        }
        if self.has_control_chars {
            flags.push("제어문자");
        }
        if flags.is_empty() {
            "이상 없음".to_string()
        } else {
            flags.join(", ")
        }
    }
}

/// 텍스트에서 BOM만 제거한 새 문자열을 만든다(원본이 BOM 없이 시작하면
/// 그대로 복사).
pub fn strip_bom_prefix(text: &str) -> String {
    text.strip_prefix('\u{feff}').unwrap_or(text).to_string()
}

/// 텍스트 안의 CRLF를 모두 LF로 바꾼 새 문자열을 만든다.
pub fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// 바이트열 중 순수 ASCII 인쇄 가능 문자 비율(%)을 계산한다(비UTF-8
/// 원본을 빠르게 스크리닝할 때 쓰는 저수준 버전 — 문자 디코딩 없이
/// 바이트만 본다).
pub fn printable_ascii_byte_ratio_percent(bytes: &[u8]) -> u32 {
    if bytes.is_empty() {
        return 100;
    }
    let printable = bytes.iter().filter(|&&b| (0x20..=0x7e).contains(&b) || b == b'\n' || b == b'\r').count();
    (printable * 100 / bytes.len()) as u32
}

/// 바이트 비율이 낮아(비ASCII/제어문자 다수) UTF-8이 아닌 다른 인코딩일
/// 가능성이 높은지 추정한다.
pub fn likely_non_utf8_source(bytes: &[u8]) -> bool {
    printable_ascii_byte_ratio_percent(bytes) < 60
}

/// 텍스트 안에서 치환 문자가 등장하는 줄 번호(1부터) 목록을 찾는다(어느
/// 행이 깨졌는지 콕 집어 로그로 남길 때 쓴다).
pub fn mangled_line_numbers(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.contains('\u{fffd}'))
        .map(|(i, _)| i + 1)
        .collect()
}

/// 텍스트에서 흔한 CP949 깨짐 패턴(물음표 연속) 개수를 센다. 완벽한
/// 탐지는 아니지만, 치환 문자로 이미 깨진 것과는 다른 경로(디코더가
/// 알 수 없는 바이트를 '?'로 대체하는 경우)를 잡아낸다.
pub fn count_question_mark_runs(text: &str, min_run: usize) -> usize {
    let mut count = 0usize;
    let mut run = 0usize;
    for c in text.chars() {
        if c == '?' {
            run += 1;
        } else {
            if run >= min_run {
                count += 1;
            }
            run = 0;
        }
    }
    if run >= min_run {
        count += 1;
    }
    count
}

/// 인코딩 점검 결과를 바탕으로 배치를 그대로 진행해도 될지, 재확인이
/// 필요한지 등급으로 나눈다.
pub fn recommend_action(report: &EncodingReport) -> &'static str {
    if report.is_clean() {
        "진행"
    } else if report.mangled {
        "재확인 필요(인코딩 재변환 권장)"
    } else {
        "주의(수동 확인 권장)"
    }
}
