//! 편차/이상치(outlier) 탐지 보조 함수.
//!
//! 이미 계산된 숫자 목록에서 평균/중앙값 대비 크게 벗어난 값을 찾아내는
//! 순수 계산 함수를 모아둔다. 리포트 검토 단계에서 "이 값이 유난히
//! 튀는데 데이터 오류가 아닌가"를 빠르게 스크리닝할 때 쓴다.

/// 값 목록의 평균을 계산한다(빈 목록은 0.0).
pub fn mean(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

/// 값 목록의 중앙값을 계산한다(짝수 개면 가운데 두 값의 평균).
pub fn median(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    } else {
        sorted[mid] as f64
    }
}

/// 값이 평균으로부터 지정 배수만큼(절대 편차 기준) 벗어났는지 검사한다.
pub fn is_outlier_by_mean(value: i64, values: &[i64], deviation_multiplier: f64) -> bool {
    let m = mean(values);
    let avg_abs_deviation = if values.is_empty() {
        0.0
    } else {
        values.iter().map(|v| (*v as f64 - m).abs()).sum::<f64>() / values.len() as f64
    };
    (value as f64 - m).abs() > avg_abs_deviation * deviation_multiplier
}

/// 값 목록에서 평균 대비 이상치로 판정되는 값만 걸러낸다.
pub fn outliers(values: &[i64], deviation_multiplier: f64) -> Vec<i64> {
    values.iter().copied().filter(|v| is_outlier_by_mean(*v, values, deviation_multiplier)).collect()
}

/// 값 목록에서 중앙값과의 차이(편차) 목록을 계산한다.
pub fn deviations_from_median(values: &[i64]) -> Vec<f64> {
    let m = median(values);
    values.iter().map(|v| *v as f64 - m).collect()
}

/// 값 목록의 범위(최댓값 - 최솟값)를 계산한다. 빈 목록은 0.
pub fn range(values: &[i64]) -> i64 {
    match (values.iter().max(), values.iter().min()) {
        (Some(max), Some(min)) => max - min,
        _ => 0,
    }
}

/// 값 목록에서 사분위수 근사값(25%, 50%, 75% 지점)을 계산한다.
///
/// 정확한 보간 방식 대신 정렬 후 인덱스로 근사하는 단순 구현이다.
pub fn approximate_quartiles(values: &[i64]) -> (f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let len = sorted.len();
    let q1 = sorted[len / 4] as f64;
    let q2 = median(&sorted);
    let q3 = sorted[(len * 3) / 4] as f64;
    (q1, q2, q3)
}

/// 사분위수 범위(IQR = Q3 - Q1)를 계산한다(이상치 탐지의 표준 지표).
pub fn interquartile_range(values: &[i64]) -> f64 {
    let (q1, _, q3) = approximate_quartiles(values);
    q3 - q1
}

/// IQR 기준(Q1 - 1.5*IQR, Q3 + 1.5*IQR 범위 밖)으로 이상치를 판정한다.
pub fn is_outlier_by_iqr(value: i64, values: &[i64]) -> bool {
    let (q1, _, q3) = approximate_quartiles(values);
    let iqr = q3 - q1;
    let lower = q1 - 1.5 * iqr;
    let upper = q3 + 1.5 * iqr;
    (value as f64) < lower || (value as f64) > upper
}

/// 값 목록에서 0에 가장 가까운 값을 찾는다(변화가 거의 없었던 항목 탐색용).
pub fn closest_to_zero(values: &[i64]) -> Option<i64> {
    values.iter().copied().min_by_key(|v| v.abs())
}

/// 값 목록의 변동 계수(표준편차/평균, 백분율)를 근사 계산한다. 평균이
/// 0이면 0을 반환한다(0으로 나누기 방지).
pub fn coefficient_of_variation_percent(values: &[i64]) -> f64 {
    let m = mean(values);
    if m == 0.0 {
        return 0.0;
    }
    let variance = values.iter().map(|v| (*v as f64 - m).powi(2)).sum::<f64>() / values.len().max(1) as f64;
    (variance.sqrt() / m.abs()) * 100.0
}
