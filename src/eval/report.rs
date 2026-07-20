//! 평가 리포트 — 실행 레코드 집계, 한국어 표, report.json (스펙 §8).

use serde::Serialize;

/// 실행 1회의 결말. Timeout은 하네스 타임아웃(run_bounded)이며,
/// 어떤 결말이든 check는 실행된다 (설계 결정 — MaxTurns라도 작업이 됐으면 통과)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Finished,
    MaxTurns,
    RepetitionStop,
    ParseFailed,
    Timeout,
}

/// 측정 조건 재현용 유효 설정 스냅샷 (스펙 M5 §4.3). model은 Report 최상위에 이미 있음.
/// api_key·auto_deny_patterns는 판정에 영향 없어 제외(비밀 유출 방지 겸)
#[derive(Debug, Serialize)]
pub struct EffectiveConfig {
    pub base_url: String,
    pub temperature: f32,
    pub context_tokens: usize,
    pub max_output_tokens: usize,
    pub max_turns: usize,
    pub command_timeout_secs: u64,
    pub loco_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    pub repeat: usize,
    /// base_seed + repeat — 개별 실행 재현용 (스펙 §8)
    pub seed: u64,
    pub passed: bool,
    pub outcome: RunOutcome,
    pub turns: usize,
    /// 에이전트 실행 시간(agent.run)만 — 판정 check·샌드박스 준비 제외 (M7 §4)
    pub duration_secs: f64,
    /// json_schema 폴백이 이 런에서 발동했는가 — true면 그 런은 스키마 강제 없이
    /// 돈 것이라 측정값으로 신뢰할 수 없다 (M13 스펙 §3-6-1 기계 검사)
    pub schema_fallback: bool,
}

#[derive(Debug, Serialize)]
pub struct TaskReport {
    pub name: String,
    pub pass_rate: f64,
    /// check 통과 실행 수 (주 지표 — per-run passed의 합, M6 §5)
    pub passed_count: usize,
    /// outcome==finished 이면서 passed — 종료 규율 지표 (M6 §5)
    pub passed_strict_count: usize,
    /// outcome==finished 인데 !passed — "자신 있는 오답" 지표 (M6 §5)
    pub false_finish_count: usize,
    /// schema_fallback==true 인 런 수 — 앵커 게이트가 전수 순회를 스크립트에
    /// 맡기지 않고 여기서 이미 집계해 fail-open 위험을 없앤다 (M14 B-3)
    pub schema_fallback_count: usize,
    pub avg_turns: f64,
    pub avg_duration_secs: f64,
    pub runs: Vec<RunRecord>,
}

impl TaskReport {
    pub fn from_runs(name: String, runs: Vec<RunRecord>) -> TaskReport {
        let n = runs.len().max(1) as f64;
        TaskReport {
            pass_rate: runs.iter().filter(|r| r.passed).count() as f64 / n,
            passed_count: runs.iter().filter(|r| r.passed).count(),
            passed_strict_count: runs
                .iter()
                .filter(|r| r.passed && r.outcome == RunOutcome::Finished)
                .count(),
            false_finish_count: runs
                .iter()
                .filter(|r| !r.passed && r.outcome == RunOutcome::Finished)
                .count(),
            schema_fallback_count: runs.iter().filter(|r| r.schema_fallback).count(),
            avg_turns: runs.iter().map(|r| r.turns as f64).sum::<f64>() / n,
            avg_duration_secs: runs.iter().map(|r| r.duration_secs).sum::<f64>() / n,
            name,
            runs,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub model: String,
    pub base_seed: u64,
    pub repeats: usize,
    pub timeout_scale: f64,
    pub started_at: String,
    /// 하네스 벽시계 총합 — check 실행·샌드박스 준비 오버헤드 포함 (M7 §4 의미 구분)
    pub duration_secs: f64,
    /// Ctrl+C로 중단된 부분 결과인지 — 표와 종료 코드(1)에 반영
    pub interrupted: bool,
    pub tasks: Vec<TaskReport>,
    pub total_pass_rate: f64,
    pub passed_count: usize,
    pub passed_strict_count: usize,
    pub false_finish_count: usize,
    /// 과제별 schema_fallback_count의 합 — 앵커 게이트가 한 번에 확인 (M14 B-3)
    pub schema_fallback_count: usize,
    /// 런당 에이전트 실행 시간의 **런 가중** 평균 — per-run `duration_secs`(agent.run만
    /// 계측, check 제외) 정의 승계라 벽시계 `duration_secs`/총런수와 일치하지 않는다 (M7 §4)
    pub avg_duration_secs: f64,
    pub effective_config: EffectiveConfig,
}

impl Report {
    /// 전체 통과율 = 통과 실행 수 / 전체 실행 수 (과제별 평균의 평균이 아님 —
    /// 중단으로 반복 수가 다른 과제가 있어도 왜곡되지 않는 정의)
    pub fn total_of(tasks: &[TaskReport]) -> f64 {
        let total: usize = tasks.iter().map(|t| t.runs.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let passed: usize = tasks.iter().map(|t| t.runs.iter().filter(|r| r.passed).count()).sum();
        passed as f64 / total as f64
    }

    /// 런 가중 평균 s/런 — total_of와 같은 정의 철학 (반복 수가 달라도 왜곡 없음, M7 §4)
    pub fn avg_duration_of(tasks: &[TaskReport]) -> f64 {
        let total: usize = tasks.iter().map(|t| t.runs.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let sum: f64 = tasks.iter().flat_map(|t| t.runs.iter().map(|r| r.duration_secs)).sum();
        sum / total as f64
    }

    /// stdout용 한국어 표 (스펙 §8 리포트). 폭 계산이 char 수 기준이라 한글
    /// 헤더(전각)와 ASCII 행의 열이 약간 어긋난다 — 과제명이 ASCII라 수용(의도적)
    pub fn render_table(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{:<28} {:>7} {:>7} {:>9} {:>10}\n",
            "과제", "통과", "엄격", "평균 턴", "평균 시간"
        ));
        for t in &self.tasks {
            let n = t.runs.len();
            out.push_str(&format!(
                "{:<28} {:>7} {:>7} {:>9.1} {:>9.1}s\n",
                t.name,
                format!("{}/{n}", t.passed_count),
                format!("{}/{n}", t.passed_strict_count),
                t.avg_turns,
                t.avg_duration_secs
            ));
        }
        let total: usize = self.tasks.iter().map(|t| t.runs.len()).sum();
        let strict_rate = if total == 0 { 0.0 } else { self.passed_strict_count as f64 / total as f64 };
        out.push_str(&format!(
            "전체 통과율 {:.1}% ({}/{total}) · 엄격 {:.1}% ({}/{total}) · 거짓 성공 finish {} · 평균 {:.1}s/런 (시드 {}부터, timeout×{}){}\n",
            self.total_pass_rate * 100.0,
            self.passed_count,
            strict_rate * 100.0,
            self.passed_strict_count,
            self.false_finish_count,
            self.avg_duration_secs,
            self.base_seed,
            self.timeout_scale,
            if self.interrupted { " — 중단됨(부분 결과)" } else { "" }
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(passed: bool, turns: usize, secs: f64) -> RunRecord {
        RunRecord {
            repeat: 0, seed: 0, passed, outcome: RunOutcome::Finished, turns,
            duration_secs: secs, schema_fallback: false,
        }
    }

    fn run_with(passed: bool, outcome: RunOutcome, schema_fallback: bool) -> RunRecord {
        RunRecord {
            repeat: 0, seed: 0, passed, outcome, turns: 1,
            duration_secs: 1.0, schema_fallback,
        }
    }

    #[test]
    fn top_level_avg_duration_is_run_weighted() {
        // 런 가중 — 과제별 평균의 평균이 아님 (M7 §4): (10+20+60)/3 = 30, 평균의 평균이면 37.5
        let a = TaskReport::from_runs("a".into(), vec![run(true, 1, 10.0), run(true, 1, 20.0)]);
        let b = TaskReport::from_runs("b".into(), vec![run(false, 1, 60.0)]);
        assert_eq!(Report::avg_duration_of(&[a, b]), 30.0);
        assert_eq!(Report::avg_duration_of(&[]), 0.0, "빈 목록은 0 (0나눗셈 금지)");
    }

    #[test]
    fn table_shows_avg_duration_per_run() {
        // sample_report는 38.5s 런 1개 — 요약 라인에 평균 s/런 노출 (M7 §4)
        let table = sample_report().render_table();
        assert!(table.contains("평균 38.5s/런"), "{table}");
    }

    #[test]
    fn strict_and_false_finish_counts() {
        let t = TaskReport::from_runs(
            "t".into(),
            vec![
                run_with(true, RunOutcome::Finished, false),   // passed + strict
                run_with(true, RunOutcome::MaxTurns, false),   // passed, 비엄격 (관대 채점의 대상)
                run_with(false, RunOutcome::Finished, false),  // 거짓 성공 finish
                run_with(false, RunOutcome::Timeout, false),   // 그냥 실패
            ],
        );
        assert_eq!(t.passed_count, 2);
        assert_eq!(t.passed_strict_count, 1, "Finished이면서 passed만");
        assert_eq!(t.false_finish_count, 1, "Finished인데 !passed만");
    }

    #[test]
    fn schema_fallback_count_aggregates_like_the_other_count_fields() {
        // M14 B-3: 앵커 게이트가 tasks[].runs[].schema_fallback을 전수 순회해야
        // 하는데, 그 스크립트를 잘못 짜면(첫 런만 본다든지) fail-open 위험이
        // Rust에서 스크립트로 이동할 뿐이다 — passed_count류와 같은 자리에 집계한다
        let runs = vec![
            run_with(true, RunOutcome::Finished, true),
            run_with(true, RunOutcome::Finished, false),
            run_with(false, RunOutcome::RepetitionStop, true),
        ];
        let t = TaskReport::from_runs("t".into(), runs);
        assert_eq!(t.schema_fallback_count, 2);
    }

    #[test]
    fn report_json_adds_count_fields_keeps_old_ones() {
        let v = serde_json::to_value(sample_report()).unwrap();
        // 신규 집계 — 과제별 + 최상위 (M6 §5, _count 접미사로 기존 passed/pass_rate와 충돌 회피)
        for key in ["passed_count", "passed_strict_count", "false_finish_count"] {
            assert!(v["tasks"][0].get(key).is_some(), "TaskReport에 {key}");
            assert!(v.get(key).is_some(), "Report 최상위에 {key}");
        }
        // 하위 호환 — 기존 키 이름·의미 불변
        assert!(v["tasks"][0].get("pass_rate").is_some());
        assert!(v.get("total_pass_rate").is_some());
        assert_eq!(v["tasks"][0]["runs"][0]["passed"], true);
    }

    #[test]
    fn report_json_carries_schema_fallback_count_at_both_levels() {
        let v = serde_json::to_value(sample_report()).unwrap();
        assert!(v.get("schema_fallback_count").is_some(), "최상위 집계가 없다");
        assert!(v["tasks"][0].get("schema_fallback_count").is_some(), "과제별 집계가 없다");
        // 기존 키 불변 — RunRecord::schema_fallback (report_json_has_design_schema_fields가
        // 같은 필드를 이미 핀하고 있어 여기서는 존재만 재확인)
        assert!(v["tasks"][0]["runs"][0].get("schema_fallback").is_some());
    }

    #[test]
    fn table_shows_strict_column_and_false_finish_summary() {
        let tasks = vec![TaskReport::from_runs(
            "demo".into(),
            vec![run_with(true, RunOutcome::MaxTurns, false), run_with(false, RunOutcome::Finished, false)],
        )];
        let mut r = sample_report();
        r.total_pass_rate = Report::total_of(&tasks);
        r.passed_count = tasks.iter().map(|t| t.passed_count).sum();
        r.passed_strict_count = tasks.iter().map(|t| t.passed_strict_count).sum();
        r.false_finish_count = tasks.iter().map(|t| t.false_finish_count).sum();
        r.tasks = tasks;
        let table = r.render_table();
        assert!(table.contains("엄격"), "{table}");
        assert!(table.contains("거짓 성공 finish 1"), "{table}");
    }

    #[test]
    fn from_runs_computes_averages() {
        let t = TaskReport::from_runs("t".into(), vec![run(true, 4, 10.0), run(false, 6, 20.0)]);
        assert_eq!(t.pass_rate, 0.5);
        assert_eq!(t.avg_turns, 5.0);
        assert_eq!(t.avg_duration_secs, 15.0);
    }

    #[test]
    fn empty_runs_do_not_divide_by_zero() {
        let t = TaskReport::from_runs("t".into(), vec![]);
        assert_eq!(t.pass_rate, 0.0);
        assert_eq!(Report::total_of(&[t]), 0.0);
    }

    #[test]
    fn total_is_runs_weighted() {
        let a = TaskReport::from_runs("a".into(), vec![run(true, 1, 1.0)]); // 1/1
        let b = TaskReport::from_runs("b".into(), vec![run(false, 1, 1.0), run(false, 1, 1.0), run(false, 1, 1.0)]); // 0/3
        assert_eq!(Report::total_of(&[a, b]), 0.25, "실행 가중 — 과제 평균의 평균(0.5)이 아님");
    }

    #[test]
    fn outcome_serializes_snake_case() {
        assert_eq!(serde_json::to_value(RunOutcome::MaxTurns).unwrap(), "max_turns");
        assert_eq!(serde_json::to_value(RunOutcome::Timeout).unwrap(), "timeout");
    }

    fn sample_report() -> Report {
        let tasks = vec![TaskReport::from_runs("add-function".into(), vec![run(true, 5, 38.5)])];
        Report {
            model: "gemma-4b".into(),
            base_seed: 0,
            repeats: 1,
            timeout_scale: 1.0,
            started_at: "20260703T000000Z".into(),
            duration_secs: 40.0,
            interrupted: false,
            total_pass_rate: Report::total_of(&tasks),
            avg_duration_secs: Report::avg_duration_of(&tasks),
            passed_count: 1,
            passed_strict_count: 1,
            false_finish_count: 0,
            schema_fallback_count: 0,
            tasks,
            effective_config: EffectiveConfig {
                base_url: "http://localhost:1234/v1".into(),
                temperature: 0.1,
                context_tokens: 8192,
                max_output_tokens: 2048,
                max_turns: 25,
                command_timeout_secs: 60,
                loco_version: "test".into(),
            },
        }
    }

    #[test]
    fn report_json_has_design_schema_fields() {
        let v = serde_json::to_value(sample_report()).unwrap();
        for key in ["model", "base_seed", "repeats", "timeout_scale", "started_at", "duration_secs", "interrupted", "tasks", "total_pass_rate", "effective_config", "avg_duration_secs"] {
            assert!(v.get(key).is_some(), "리포트에 {key} 필드가 있어야 함");
        }
        assert_eq!(v["tasks"][0]["runs"][0]["seed"], 0, "시드 기록 (스펙 §8 재현성)");
        assert_eq!(v["tasks"][0]["runs"][0]["outcome"], "finished");
        assert!(v["tasks"][0]["runs"][0].get("schema_fallback").is_some(), "RunRecord에 schema_fallback 필드가 있어야 함 (M13 스펙 §3-6-1)");
    }

    #[test]
    fn table_mentions_tasks_and_total() {
        let table = sample_report().render_table();
        assert!(table.contains("add-function"));
        assert!(table.contains("1/1"));
        assert!(table.contains("전체 통과율 100.0%"));
        assert!(!table.contains("중단됨"));
        let mut interrupted = sample_report();
        interrupted.interrupted = true;
        assert!(interrupted.render_table().contains("중단됨"));
    }

    #[test]
    fn report_json_snapshots_effective_config() {
        let v = serde_json::to_value(sample_report()).unwrap();
        let ec = v.get("effective_config").expect("유효 config 스냅샷 (스펙 M5 §4.3)");
        for key in ["base_url", "temperature", "context_tokens", "max_output_tokens", "max_turns", "command_timeout_secs", "loco_version"] {
            assert!(ec.get(key).is_some(), "effective_config에 {key}");
        }
    }
}
