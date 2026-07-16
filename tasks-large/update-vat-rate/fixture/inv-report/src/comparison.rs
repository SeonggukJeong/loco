//! 두 기간(또는 두 배치)의 결과를 비교하는 보조 함수.
//!
//! 이미 계산된 합계 값 두 개를 받아 차이/비율을 계산하는 순수 함수들이다.
//! 어느 버전의 합계 함수로 값을 만들었는지는 이 모듈이 신경 쓰지 않는다
//! (v1이든 v2든 이미 계산된 숫자만 받는다).

/// 비교 결과 한 건.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub baseline: i64,
    pub current: i64,
}

impl Comparison {
    pub fn new(baseline: i64, current: i64) -> Self {
        Comparison { baseline, current }
    }

    /// 절대 증감액(현재 - 기준).
    pub fn delta(&self) -> i64 {
        self.current - self.baseline
    }

    /// 증감률(%). 기준값이 0이면 0을 반환한다(0으로 나누기 방지).
    pub fn delta_percent(&self) -> i64 {
        if self.baseline == 0 {
            return 0;
        }
        (self.delta() * 100) / self.baseline
    }

    /// 개선(증가) 여부.
    pub fn improved(&self) -> bool {
        self.delta() > 0
    }

    /// 변화가 없는지(정확히 동일한지) 여부.
    pub fn unchanged(&self) -> bool {
        self.delta() == 0
    }
}

/// 비교 결과를 사람이 읽는 한 줄로 포맷한다.
pub fn format_comparison(cmp: &Comparison) -> String {
    let sign = if cmp.delta() >= 0 { "+" } else { "" };
    format!("{} -> {} ({sign}{}, {sign}{}%)", cmp.baseline, cmp.current, cmp.delta(), cmp.delta_percent())
}

/// 여러 (기준, 현재) 쌍을 한 번에 비교 결과로 변환한다.
pub fn compare_many(pairs: &[(i64, i64)]) -> Vec<Comparison> {
    pairs.iter().map(|(b, c)| Comparison::new(*b, *c)).collect()
}

/// 비교 결과 목록 중 개선폭(delta)이 가장 큰 것을 찾는다.
pub fn best_improvement(comparisons: &[Comparison]) -> Option<&Comparison> {
    comparisons.iter().max_by_key(|c| c.delta())
}

/// 비교 결과 목록 중 악화폭이 가장 큰 것을 찾는다.
pub fn worst_decline(comparisons: &[Comparison]) -> Option<&Comparison> {
    comparisons.iter().min_by_key(|c| c.delta())
}

/// 비교 결과 목록 중 개선된 것의 개수를 센다.
pub fn improved_count(comparisons: &[Comparison]) -> usize {
    comparisons.iter().filter(|c| c.improved()).count()
}

/// 비교 결과 목록의 평균 증감률(%)을 계산한다(빈 목록은 0.0).
pub fn average_delta_percent(comparisons: &[Comparison]) -> f64 {
    if comparisons.is_empty() {
        return 0.0;
    }
    comparisons.iter().map(|c| c.delta_percent()).sum::<i64>() as f64 / comparisons.len() as f64
}

/// 세 값(전전기/전기/현재)으로 2연속 개선(가속) 여부를 판정한다.
pub fn is_accelerating(two_periods_ago: i64, previous: i64, current: i64) -> bool {
    let first = Comparison::new(two_periods_ago, previous);
    let second = Comparison::new(previous, current);
    first.improved() && second.improved() && second.delta() >= first.delta()
}

/// 허용 오차 이내로 두 값이 실질적으로 같은지 비교한다(반올림 오차 흡수용).
pub fn nearly_equal(a: i64, b: i64, tolerance: i64) -> bool {
    (a - b).abs() <= tolerance
}

/// 여러 비교 결과를 값 개선폭 내림차순으로 정렬한다.
pub fn sort_by_delta_desc(comparisons: &mut Vec<Comparison>) {
    comparisons.sort_by_key(|c| std::cmp::Reverse(c.delta()));
}

/// 비교 결과 목록 중 변화가 없는(unchanged) 것의 개수를 센다.
pub fn unchanged_count(comparisons: &[Comparison]) -> usize {
    comparisons.iter().filter(|c| c.unchanged()).count()
}

/// 비교 결과 목록 중 악화된(delta가 음수인) 것의 개수를 센다.
pub fn declined_count(comparisons: &[Comparison]) -> usize {
    comparisons.iter().filter(|c| c.delta() < 0).count()
}

/// 세 개 이상의 값으로 이루어진 시계열에서 인접한 값끼리 순서대로 비교
/// 결과 목록을 만든다.
pub fn compare_sequential(series: &[i64]) -> Vec<Comparison> {
    series.windows(2).map(|w| Comparison::new(w[0], w[1])).collect()
}

/// 비교 결과 목록의 delta 절대값 합계를 구한다(변동성 지표).
pub fn total_absolute_delta(comparisons: &[Comparison]) -> i64 {
    comparisons.iter().map(|c| c.delta().abs()).sum()
}

/// 두 값을 비교해 개선률이 지정 임계값(%) 이상인지 판정한다.
pub fn exceeds_improvement_threshold(baseline: i64, current: i64, threshold_percent: i64) -> bool {
    Comparison::new(baseline, current).delta_percent() >= threshold_percent
}
