//! 셸 명령 실행 공용 기반 — run_command 툴과 eval check가 공유 (스펙 §10).
//! 프로세스 그룹 킬, UTF-8/CP949 디코딩, 출력 중간 절삭.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// stdout+stderr 합산 상한 (바이트). 초과분은 가운데를 잘라낸다 —
/// 명령 에코는 앞에, 에러 요약은 뒤에 있는 경우가 많다
const MAX_OUTPUT_BYTES: usize = 8_000;
/// try_wait 폴링 간격
const POLL: Duration = Duration::from_millis(50);
/// 종료 판정 후 파이프 리더 대기 상한. join()은 금지 — 백그라운드 손자가
/// 파이프를 물고 있으면(`sh -c "x &"` 또는 그룹 킬 실패) EOF가 영원히 안 와서
/// 툴이 무한 대기한다. 상한 초과 시 해당 출력은 포기하고 안내를 남긴다
const READER_GRACE: Duration = Duration::from_millis(500);

/// UTF-8 우선, 실패 시 EUC-KR(windows-949) 손실 디코딩 (스펙 §10 — 한국어 Windows 콘솔)
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

fn truncate_middle(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let mut head = MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = s.len() - MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n[... output truncated ({} bytes total) ...]\n{}",
        &s[..head],
        s.len(),
        &s[tail..]
    )
}

/// 파이프 리더 — 결과를 채널로 보낸다. JoinHandle::join 대신 recv_timeout을
/// 쓸 수 있게 하기 위함 (READER_GRACE 주석 참조)
fn spawn_reader(r: Option<impl Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// (디코딩된 출력, 제시간에 EOF를 받았는지)
fn drain(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> (String, bool) {
    match rx.recv_timeout(READER_GRACE) {
        Ok(bytes) => (decode(&bytes), true),
        Err(_) => (String::new(), false),
    }
}

fn shell_command(command: &str, cwd: &Path) -> Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("sh");
        c.arg("-c").arg(command).current_dir(cwd);
        // 자기만의 프로세스 그룹 — 타임아웃 킬이 손자까지 잡게 (스펙 §10)
        c.process_group(0);
        c
    }
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", command]).current_dir(cwd);
        c
    }
}

/// 프로세스 트리 킬 (스펙 §10). 신규 크레이트 없이 시스템 유틸로:
/// Unix는 프로세스 그룹에 kill -9, Windows는 taskkill /T /F
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").args(["-9", "--", &format!("-{}", child.id())]).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).status();
    }
    let _ = child.kill(); // 그룹 킬 실패 대비 직접 킬
}

pub enum ExecEnd {
    Done(std::process::ExitStatus),
    TimedOut,
    Cancelled,
}

pub struct Exec {
    pub end: ExecEnd,
    /// "--- stdout ---"/"--- stderr ---" 섹션 + 절삭·파이프 점유 안내가 적용된 본문
    pub body: String,
}

pub fn exec_shell(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> std::io::Result<Exec> {
    let mut child = shell_command(command, cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let out_rx = spawn_reader(child.stdout.take());
    let err_rx = spawn_reader(child.stderr.take());

    let start = Instant::now();
    let end = loop {
        if let Some(status) = child.try_wait()? {
            break ExecEnd::Done(status);
        }
        if cancel.load(Ordering::SeqCst) {
            kill_tree(&mut child);
            let _ = child.wait();
            break ExecEnd::Cancelled;
        }
        if start.elapsed() >= timeout {
            kill_tree(&mut child);
            let _ = child.wait();
            break ExecEnd::TimedOut;
        }
        std::thread::sleep(POLL);
    };

    // 기존 run_command::run의 출력 조립 로직 그대로 (stdout/stderr 섹션,
    // 파이프 점유 안내, truncate_middle)
    let (stdout, out_ok) = drain(out_rx);
    let (stderr, err_ok) = drain(err_rx);
    let mut body = String::new();
    if !stdout.trim().is_empty() {
        body.push_str("--- stdout ---\n");
        body.push_str(&stdout);
    }
    if !stderr.trim().is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("--- stderr ---\n");
        body.push_str(&stderr);
    }
    if !out_ok || !err_ok {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("(some output unavailable - a background child still holds the pipe)");
    }
    Ok(Exec { end, body: truncate_middle(&body) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_falls_back_to_euc_kr() {
        assert_eq!(decode("한글".as_bytes()), "한글");
        // "한글"의 CP949 인코딩: C7 D1 B1 DB
        assert_eq!(decode(&[0xC7, 0xD1, 0xB1, 0xDB]), "한글");
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let s = "x".repeat(20_000);
        let t = truncate_middle(&s);
        assert!(t.len() < 12_000);
        assert!(t.contains("output truncated"));
        let short = "short";
        assert_eq!(truncate_middle(short), "short");
    }

    #[cfg(unix)]
    #[test]
    fn exec_shell_reports_exit_status() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let e = exec_shell("exit 7", dir.path(), Duration::from_secs(5), &cancel).unwrap();
        assert!(matches!(e.end, ExecEnd::Done(s) if s.code() == Some(7)));
    }
}
