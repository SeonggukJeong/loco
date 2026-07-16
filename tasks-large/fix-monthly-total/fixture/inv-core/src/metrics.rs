//! 기본 통계 헬퍼(평균/표준편차/백분위수).
//!
//! 재고 회전율, 예측 오차 등 여러 보고서에서 반복적으로 쓰이는 통계
//! 연산을 한곳에 모았다. 외부 통계 크레이트 의존 없이 직접 구현한다.

/// 평균을 계산한다(빈 슬라이스면 0.0).
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// 모집단 분산을 계산한다(빈 슬라이스면 0.0).
pub fn variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let m = mean(values);
    values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64
}

/// 표준편차를 계산한다.
pub fn stddev(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// 정렬된 값 목록에서 백분위수를 계산한다(선형 보간, p는 0~100).
///
/// 입력이 정렬되어 있지 않으면 내부에서 복사본을 정렬해 계산한다.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p = p.clamp(0.0, 100.0);
    let rank = (p / 100.0) * (sorted.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = rank - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// 중앙값(50번째 백분위수)을 계산한다.
pub fn median(values: &[f64]) -> f64 {
    percentile(values, 50.0)
}

/// 최솟값을 찾는다(빈 슬라이스면 `None`).
pub fn min_of(values: &[f64]) -> Option<f64> {
    values.iter().cloned().fold(None, |acc, v| match acc {
        None => Some(v),
        Some(m) => Some(if v < m { v } else { m }),
    })
}

/// 최댓값을 찾는다(빈 슬라이스면 `None`).
pub fn max_of(values: &[f64]) -> Option<f64> {
    values.iter().cloned().fold(None, |acc, v| match acc {
        None => Some(v),
        Some(m) => Some(if v > m { v } else { m }),
    })
}

/// 평균절대오차(MAE)를 계산한다(예측값과 실제값 목록의 길이가 같아야 함).
pub fn mean_absolute_error(forecast: &[f64], actual: &[f64]) -> f64 {
    let n = forecast.len().min(actual.len());
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = (0..n).map(|i| (forecast[i] - actual[i]).abs()).sum();
    sum / n as f64
}

/// 평균제곱오차(MSE)를 계산한다.
pub fn mean_squared_error(forecast: &[f64], actual: &[f64]) -> f64 {
    let n = forecast.len().min(actual.len());
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = (0..n).map(|i| (forecast[i] - actual[i]).powi(2)).sum();
    sum / n as f64
}

/// 평균절대백분율오차(MAPE, %)를 계산한다. 실제값이 0인 항목은 계산에서
/// 제외한다(0으로 나누기 방지).
pub fn mean_absolute_percentage_error(forecast: &[f64], actual: &[f64]) -> f64 {
    let n = forecast.len().min(actual.len());
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..n {
        if actual[i] != 0.0 {
            sum += ((forecast[i] - actual[i]) / actual[i]).abs();
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        (sum / count as f64) * 100.0
    }
}

/// 값 목록을 0~1 범위로 min-max 정규화한다(모든 값이 같으면 전부 0.0).
pub fn normalize_min_max(values: &[f64]) -> Vec<f64> {
    let min = min_of(values).unwrap_or(0.0);
    let max = max_of(values).unwrap_or(0.0);
    let range = max - min;
    if range == 0.0 {
        return vec![0.0; values.len()];
    }
    values.iter().map(|v| (v - min) / range).collect()
}

/// 값 목록의 누적합(running total)을 계산한다.
pub fn cumulative_sum(values: &[f64]) -> Vec<f64> {
    let mut total = 0.0;
    values
        .iter()
        .map(|v| {
            total += v;
            total
        })
        .collect()
}

/// 이동평균(단순, window 크기)을 계산한다. 앞쪽 window-1개 항목은
/// 그때까지의 부분 평균으로 채운다.
pub fn simple_moving_average(values: &[f64], window: usize) -> Vec<f64> {
    if window == 0 {
        return values.to_vec();
    }
    (0..values.len())
        .map(|i| {
            let start = i.saturating_sub(window - 1);
            mean(&values[start..=i])
        })
        .collect()
}

/// 두 값 목록의 상관관계를 대략적으로 나타내는 부호(같은 방향으로 움직이는
/// 비율)를 계산한다: 1.0에 가까울수록 같은 방향, -1.0에 가까울수록 반대
/// 방향으로 움직인 경우가 많다는 뜻이다(정식 피어슨 상관계수는 아님).
pub fn co_movement_score(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let mut same_direction = 0i64;
    let mut total = 0i64;
    for i in 1..n {
        let da = a[i] - a[i - 1];
        let db = b[i] - b[i - 1];
        if da != 0.0 && db != 0.0 {
            total += 1;
            if (da > 0.0) == (db > 0.0) {
                same_direction += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        (2.0 * same_direction as f64 / total as f64) - 1.0
    }
}
