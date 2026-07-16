//! 시계열(추이) 분석 보조 함수.
//!
//! 이미 계산된 숫자 시계열(월별 합계, 전망치 등)을 받아 증감/방향성을
//! 판단하는 순수 계산 함수를 모아둔다. 원장 라인을 직접 다루지 않고
//! 정수 시계열만 입력받는 범용 형태라서, 순매출/라인 수/재고량 등 어떤
//! 값의 시계열이든 동일하게 쓸 수 있다.

/// 시계열이 단조 증가(각 값이 이전 값 이상)인지 검사한다.
pub fn is_monotonic_increasing(series: &[i64]) -> bool {
    series.windows(2).all(|w| w[1] >= w[0])
}

/// 시계열이 단조 감소인지 검사한다.
pub fn is_monotonic_decreasing(series: &[i64]) -> bool {
    series.windows(2).all(|w| w[1] <= w[0])
}

/// 연속된 두 값 사이의 증감량 목록을 계산한다(길이는 원본보다 1 짧다).
pub fn deltas(series: &[i64]) -> Vec<i64> {
    series.windows(2).map(|w| w[1] - w[0]).collect()
}

/// 시계열에서 가장 큰 단일 증가폭을 찾는다.
pub fn largest_increase(series: &[i64]) -> Option<i64> {
    deltas(series).into_iter().filter(|d| *d > 0).max()
}

/// 시계열에서 가장 큰 단일 감소폭(절대값)을 찾는다.
pub fn largest_decrease(series: &[i64]) -> Option<i64> {
    deltas(series).into_iter().filter(|d| *d < 0).map(|d| d.abs()).max()
}

/// 시계열에서 연속으로 증가한 최장 구간의 길이를 계산한다(값 개수 기준,
/// 증가가 전혀 없으면 시계열 길이가 0/1일 때만 해당 길이를 그대로 반환).
pub fn longest_increasing_streak(series: &[i64]) -> usize {
    if series.len() < 2 {
        return series.len();
    }
    let mut longest = 1usize;
    let mut current = 1usize;
    for d in deltas(series) {
        if d > 0 {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 1;
        }
    }
    longest
}

/// 시계열의 값이 0을 가로지르는(음수에서 양수로, 또는 그 반대로 바뀌는)
/// 지점의 개수를 센다(적자/흑자 전환 횟수).
pub fn zero_crossings(series: &[i64]) -> usize {
    series.windows(2).filter(|w| (w[0] < 0 && w[1] >= 0) || (w[0] >= 0 && w[1] < 0)).count()
}

/// 시계열을 지정 구간 수만큼 균등 분할했을 때 각 구간의 합계를 계산한다.
///
/// 구간 수가 0이거나 시계열이 비어 있으면 빈 목록을 반환한다.
pub fn bucketed_sums(series: &[i64], buckets: usize) -> Vec<i64> {
    if buckets == 0 || series.is_empty() {
        return Vec::new();
    }
    let bucket_size = (series.len() + buckets - 1) / buckets;
    series.chunks(bucket_size.max(1)).map(|chunk| chunk.iter().sum()).collect()
}

/// 시계열의 표준편차를 계산한다(모집단 표준편차, 빈 시계열은 0.0).
pub fn standard_deviation(series: &[i64]) -> f64 {
    if series.is_empty() {
        return 0.0;
    }
    let mean = series.iter().sum::<i64>() as f64 / series.len() as f64;
    let variance = series.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / series.len() as f64;
    variance.sqrt()
}

/// 시계열이 이전 값 대비 지정 비율(%) 이상 급변한 지점의 인덱스를 찾는다.
pub fn volatile_indices(series: &[i64], threshold_percent: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, w) in series.windows(2).enumerate() {
        if w[0] == 0 {
            continue;
        }
        let change_percent = ((w[1] - w[0]).abs() * 100) / w[0].abs();
        if change_percent as u32 >= threshold_percent {
            out.push(i + 1);
        }
    }
    out
}

/// 시계열을 정규화(최댓값을 100으로)한 백분율 시계열로 변환한다.
///
/// 원본 최댓값이 0 이하이면 전부 0으로 채운 시계열을 반환한다.
pub fn normalize_to_max_100(series: &[i64]) -> Vec<i64> {
    let max = series.iter().copied().max().unwrap_or(0);
    if max <= 0 {
        return vec![0; series.len()];
    }
    series.iter().map(|v| (v * 100) / max).collect()
}

/// 시계열에서 연속으로 감소한 최장 구간의 길이를 계산한다.
pub fn longest_decreasing_streak(series: &[i64]) -> usize {
    if series.len() < 2 {
        return series.len();
    }
    let mut longest = 1usize;
    let mut current = 1usize;
    for d in deltas(series) {
        if d < 0 {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 1;
        }
    }
    longest
}

/// 시계열의 값이 전부 같은(변동이 없는) 구간인지 검사한다.
pub fn is_flat(series: &[i64]) -> bool {
    series.windows(2).all(|w| w[0] == w[1])
}

/// 시계열의 누적 최댓값 시계열을 계산한다(각 지점까지의 최고치).
pub fn running_max(series: &[i64]) -> Vec<i64> {
    let mut current_max = i64::MIN;
    series
        .iter()
        .map(|v| {
            current_max = current_max.max(*v);
            current_max
        })
        .collect()
}

/// 시계열이 직전 최고점(running max) 대비 얼마나 하락했는지(고점 대비
/// 낙폭, drawdown)를 지점별로 계산한다.
pub fn drawdown_series(series: &[i64]) -> Vec<i64> {
    let peaks = running_max(series);
    series.iter().zip(peaks.iter()).map(|(v, peak)| peak - v).collect()
}

/// 시계열의 최대 낙폭(최대 drawdown)을 계산한다.
pub fn max_drawdown(series: &[i64]) -> i64 {
    drawdown_series(series).into_iter().max().unwrap_or(0)
}

/// 두 시계열의 지점별 차이(a - b)를 계산한다. 길이가 다르면 짧은 쪽까지만
/// 비교한다.
pub fn pairwise_diff(a: &[i64], b: &[i64]) -> Vec<i64> {
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// 시계열의 합계를 구간 수로 나눈 평균을 계산한다(빈 시계열은 0.0).
pub fn series_average(series: &[i64]) -> f64 {
    if series.is_empty() {
        0.0
    } else {
        series.iter().sum::<i64>() as f64 / series.len() as f64
    }
}
