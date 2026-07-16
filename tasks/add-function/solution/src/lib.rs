/// 정수 슬라이스의 중앙값. 짝수 길이는 가운데 두 값의 평균.
/// 입력은 비어 있지 않다고 가정한다.
pub fn median(xs: &[i64]) -> f64 {
    let mut sorted = xs.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] as f64 + sorted[n / 2] as f64) / 2.0
    }
}
