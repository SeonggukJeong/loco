/// 쉼표로 구분된 정수 목록의 합계. 공백 허용, 빈 문자열은 0.
pub fn sum_csv(input: &str) -> i64 {
    input.split(',').map(|p| p.trim().parse::<i64>().unwrap_or(0)).sum()
}

/// 목록의 최댓값. 파싱 불가 항목은 무시, 빈 목록이면 None.
pub fn max_csv(input: &str) -> Option<i64> {
    let mut best: Option<i64> = None;
    for part in input.split(',') {
        let Ok(v) = part.trim().parse::<i64>() else { continue };
        if best.is_none() || v > best.unwrap() {
            best = Some(v);
        }
    }
    best
}
