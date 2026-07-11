//! 에이전트 실행을 시간·Ctrl+C로 경계 짓는 러너 (M4 설계 §1, 백로그 ①).

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// run_bounded가 퓨처를 중도 포기한 이유
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    TimedOut,
    Interrupted,
}

/// 플래그가 설 때까지 폴링(50ms) — eval의 공유 인터럽트 플래그를 run_bounded의
/// interrupt 퓨처로 바꾸는 어댑터
pub async fn watch_flag(flag: &AtomicBool) {
    while !flag.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 중단 신호(interrupt 퓨처 완료)와 시간 상한(limit이 Some일 때)을 감시하며 퓨처를
/// 실행한다. interrupt는 호출자가 정한다: -p는 tokio::signal::ctrl_c(), eval은
/// watch_flag(장수 리스너가 세우는 공유 플래그) — ctrl_c()는 등록 이후의 신호만
/// 보므로 select! 창 밖 구간이 있는 호출자가 그대로 쓰면 SIGINT가 유실된다.
/// 발화 시 cancel 플래그를 세우고 유예(grace) 동안 퓨처의 자연 종료를 기다린다 —
/// 즉시 드롭하면 run_command의 자식 프로세스 그룹을 죽일 기회가 없어 고아가 남는다.
/// 유예 안에 완료돼도 결과는 버린다: 호출자에게는 중단 사실이 결과보다 중요하다.
pub async fn run_bounded<F: Future, I: Future>(
    fut: F,
    cancel: &AtomicBool,
    limit: Option<Duration>,
    grace: Duration,
    interrupt: I,
) -> Result<F::Output, Stopped> {
    tokio::pin!(fut);
    tokio::pin!(interrupt);
    let stopped = tokio::select! {
        out = &mut fut => return Ok(out),
        _ = &mut interrupt => Stopped::Interrupted,
        _ = sleep_limit(limit) => Stopped::TimedOut,
    };
    cancel.store(true, Ordering::SeqCst);
    let _ = tokio::time::timeout(grace, &mut fut).await;
    Err(stopped)
}

async fn sleep_limit(limit: Option<Duration>) {
    match limit {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 완료되지 않는 interrupt — Ctrl+C가 없는 상황
    fn never() -> std::future::Pending<()> {
        std::future::pending()
    }

    #[tokio::test]
    async fn completed_future_passes_through() {
        let cancel = AtomicBool::new(false);
        let r = run_bounded(async { 42 }, &cancel, Some(Duration::from_secs(5)), Duration::from_millis(10), never()).await;
        assert_eq!(r.unwrap(), 42);
        assert!(!cancel.load(Ordering::SeqCst), "정상 완료는 플래그를 건드리지 않음");
    }

    #[tokio::test]
    async fn timeout_sets_cancel_and_reports_timed_out() {
        let cancel = AtomicBool::new(false);
        let r = run_bounded(
            std::future::pending::<()>(),
            &cancel,
            Some(Duration::from_millis(20)),
            Duration::from_millis(10),
            never(),
        )
        .await;
        assert_eq!(r.unwrap_err(), Stopped::TimedOut);
        assert!(cancel.load(Ordering::SeqCst), "타임아웃은 cancel 플래그를 세운다");
    }

    #[tokio::test]
    async fn interrupt_future_stops_and_sets_cancel() {
        let cancel = AtomicBool::new(false);
        let flag = AtomicBool::new(true); // 이미 선 플래그 — watch_flag가 즉시 완료
        let r = run_bounded(
            std::future::pending::<()>(),
            &cancel,
            None,
            Duration::from_millis(10),
            watch_flag(&flag),
        )
        .await;
        assert_eq!(r.unwrap_err(), Stopped::Interrupted);
        assert!(cancel.load(Ordering::SeqCst), "중단도 cancel 플래그를 세운다");
    }

    #[tokio::test]
    async fn grace_lets_the_future_finish_side_effects() {
        // limit(20ms) 발화 후에도 유예(1s) 동안 퓨처가 정리 작업을 마친다 — 부수효과로 관찰
        let cancel = AtomicBool::new(false);
        let cleaned = Arc::new(AtomicBool::new(false));
        let c2 = cleaned.clone();
        let fut = async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            c2.store(true, Ordering::SeqCst);
        };
        let r = run_bounded(fut, &cancel, Some(Duration::from_millis(20)), Duration::from_secs(1), never()).await;
        assert_eq!(r.unwrap_err(), Stopped::TimedOut, "결과는 버려진다");
        assert!(cleaned.load(Ordering::SeqCst), "유예 동안 자연 종료가 완료됨");
    }
}
