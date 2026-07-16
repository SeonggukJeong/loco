//! 매출 전망(forecast) 계산.
//!
//! 순매출(세전)로부터 세금 포함 전망치를 뽑아내고, 여러 기간에 걸친 성장률
//! 기반 예측 보조 함수를 담는다. 인보이스 계층과 달리 여기서는 정수 계산
//! 대신 부동소수점 배율로 세율을 반영한다(반올림 방식이 조금 다르다).

/// 순매출(세전, 원)에 10% 세율을 반영한 전망치를 계산한다.
pub fn forecast_projection(net_krw: i64) -> i64 { (net_krw as f64 * 1.10) as i64 }

/// 여러 기간(월 등)에 걸쳐 순매출이 동일하다고 가정했을 때, 기간 수만큼
/// 반복한 전망치 목록을 만든다(가장 단순한 flat 전망).
pub fn project_flat(net_krw: i64, periods: u32) -> Vec<i64> {
    (0..periods).map(|_| forecast_projection(net_krw)).collect()
}

/// 기간별 성장률(%)을 적용해 다음 기간의 순매출을 추정한다.
pub fn apply_growth_rate(net_krw: i64, growth_percent: i64) -> i64 {
    net_krw + (net_krw * growth_percent / 100)
}

/// 초기 순매출에 고정 성장률을 반복 적용해 여러 기간의 전망치 목록을 만든다.
pub fn project_with_growth(initial_net_krw: i64, growth_percent: i64, periods: u32) -> Vec<i64> {
    let mut current = initial_net_krw;
    let mut out = Vec::with_capacity(periods as usize);
    for _ in 0..periods {
        out.push(forecast_projection(current));
        current = apply_growth_rate(current, growth_percent);
    }
    out
}

/// 두 기간의 순매출로부터 성장률(%)을 역산한다. 이전 값이 0이면 0을
/// 반환한다(0으로 나누기 방지).
pub fn growth_rate_percent(previous_net_krw: i64, current_net_krw: i64) -> i64 {
    if previous_net_krw == 0 {
        return 0;
    }
    ((current_net_krw - previous_net_krw) * 100) / previous_net_krw
}

/// 순매출 시계열의 단순 이동평균을 계산한다(윈도우 크기 = window).
///
/// 데이터가 윈도우보다 짧으면 전체 평균을 반환한다.
pub fn moving_average(series: &[i64], window: usize) -> f64 {
    if series.is_empty() {
        return 0.0;
    }
    let w = window.min(series.len()).max(1);
    let recent = &series[series.len() - w..];
    recent.iter().sum::<i64>() as f64 / w as f64
}

/// 실제 값과 전망치의 오차(전망치 - 실제)를 계산한다.
pub fn forecast_variance(projected_krw: i64, actual_krw: i64) -> i64 {
    projected_krw - actual_krw
}

/// 오차율(%)을 계산한다. 실제 값이 0이면 0을 반환한다.
pub fn forecast_variance_percent(projected_krw: i64, actual_krw: i64) -> i64 {
    if actual_krw == 0 {
        return 0;
    }
    (forecast_variance(projected_krw, actual_krw) * 100) / actual_krw
}

/// 전망치가 실제 값보다 낙관적인지(과대 예측인지) 판정한다.
pub fn is_overly_optimistic(projected_krw: i64, actual_krw: i64) -> bool {
    projected_krw > actual_krw
}

/// 여러 기간의 전망 오차율 목록으로부터 평균 절대 오차율(%)을 계산한다
/// (전망 정확도 리포트에 쓰인다).
pub fn mean_absolute_percent_error(pairs: &[(i64, i64)]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let sum: i64 = pairs.iter().map(|(p, a)| forecast_variance_percent(*p, *a).abs()).sum();
    sum as f64 / pairs.len() as f64
}

/// 전망치 시계열의 누적 합계를 계산한다(분기/연간 롤업용).
pub fn cumulative_forecast(series: &[i64]) -> Vec<i64> {
    let mut running = 0i64;
    series
        .iter()
        .map(|v| {
            running += v;
            running
        })
        .collect()
}

/// 전망치 시계열에서 최댓값과 최솟값의 차이(변동폭)를 계산한다.
pub fn forecast_range(series: &[i64]) -> i64 {
    match (series.iter().max(), series.iter().min()) {
        (Some(max), Some(min)) => max - min,
        _ => 0,
    }
}

/// 목표 전망치를 달성하기 위해 필요한 순매출을 역산한다(세율 배율의 역).
pub fn required_net_for_target(target_projected_krw: i64) -> i64 {
    ((target_projected_krw as f64) / 1.10) as i64
}

/// 순매출 시계열의 평균 성장률(%)을 계산한다(연속 구간별 성장률의 평균).
pub fn average_growth_rate_percent(series: &[i64]) -> i64 {
    if series.len() < 2 {
        return 0;
    }
    let rates: Vec<i64> = series.windows(2).map(|w| growth_rate_percent(w[0], w[1])).collect();
    rates.iter().sum::<i64>() / rates.len() as i64
}

/// 순매출 시계열이 지정 기간 동안 계속 성장했는지(모든 구간이 양의
/// 성장률인지) 검사한다.
pub fn is_consistently_growing(series: &[i64]) -> bool {
    series.windows(2).all(|w| growth_rate_percent(w[0], w[1]) > 0)
}

/// 전망치 목록 중 실제 값과의 오차가 허용 범위(%) 이내인 것의 개수를 센다
/// (전망 신뢰도 리포트용).
pub fn accurate_forecast_count(pairs: &[(i64, i64)], tolerance_percent: i64) -> usize {
    pairs.iter().filter(|(p, a)| forecast_variance_percent(*p, *a).abs() <= tolerance_percent).count()
}

/// 여러 기간의 전망 오차(전망치 - 실제) 목록을 계산한다.
pub fn variance_series(pairs: &[(i64, i64)]) -> Vec<i64> {
    pairs.iter().map(|(p, a)| forecast_variance(*p, *a)).collect()
}

/// 성장률을 반복 적용했을 때 초기값이 목표값에 도달하는 데 필요한
/// 최소 기간 수를 계산한다(성장률이 0 이하면 도달 불가로 보고 `None`).
pub fn periods_to_reach_target(initial_net_krw: i64, growth_percent: i64, target_net_krw: i64) -> Option<u32> {
    if growth_percent <= 0 || initial_net_krw >= target_net_krw {
        return if initial_net_krw >= target_net_krw { Some(0) } else { None };
    }
    let mut current = initial_net_krw;
    let mut periods = 0u32;
    while current < target_net_krw && periods < 10_000 {
        current = apply_growth_rate(current, growth_percent);
        periods += 1;
    }
    if current >= target_net_krw {
        Some(periods)
    } else {
        None
    }
}

/// 전망치 시계열 중 이전 값 대비 감소한 지점의 개수를 센다(역성장 구간 수).
pub fn regression_count(series: &[i64]) -> usize {
    series.windows(2).filter(|w| w[1] < w[0]).count()
}
