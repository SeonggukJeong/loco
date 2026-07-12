# M5 스캐폴딩 개선 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 기준선 실패 분석에서 도출한 5개 메커니즘(스펙 §1)을 Batch 0~3으로 제거하고, 배치별 qwen 측정과 최종 두 모델 측정으로 효과를 검증한다.

**Architecture:** 기존 모듈 내 개선 — `eval/`(하네스 무결성), `agent/protocol.rs`(salvage 파싱), `tools/`(에러 피드백·edit_file), `agent/mod.rs`+신규 `agent/repetition.rs`(루프). 새 크레이트 없음, 툴 형태 유지.

**Tech Stack:** Rust edition 2024, serde_json(기본 피처=BTreeMap), tokio, 기존 고정 의존성만.

**Spec:** `docs/superpowers/specs/2026-07-12-m5-scaffolding-design.md` (§ 참조는 이 문서 기준. "본선 스펙"은 2026-07-02 문서)

## Global Constraints

- 브랜치 `feat/m5-scaffolding`에서 작업 (Task 1에서 생성)
- 신규 크레이트 금지 — `Cargo.toml` 의존성 변경이 필요해 보이면 중단하고 사용자에게 질문
- 게이트(태스크마다): `cargo test` 전체 통과 + `cargo clippy --all-targets -- -D warnings` 무경고
- 모델 대상 텍스트(툴 에러·시스템 프롬프트·교정 메시지)는 영어, 사용자 대상 CLI 메시지는 한국어, 주석은 한국어 관례
- 커밋은 conventional commits (제목 한국어 가능), 태스크당 1커밋
- 측정 체크포인트(Task 8/12/16/17)는 **사용자 협조 필요**(LM Studio) — 도달 시 사용자에게 알리고 대기

## File Structure

| 파일 | 변경 | 책임 |
|---|---|---|
| `src/eval/mod.rs` | 수정 | timeout 클램프, .cargo 암묵 protected+트립와이어, config 스냅샷 전달 |
| `src/eval/report.rs` | 수정 | `EffectiveConfig` 스냅샷 필드 |
| `src/agent/protocol.rs` | 수정 | Value 경유 파싱 + salvage 정규화 (`ModelTurn.salvaged`) |
| `src/tools/mod.rs` | 수정 | dispatch의 스키마 에코 래핑 |
| `src/tools/read_file.rs` | 수정 | 디렉터리 에러 번역 |
| `src/tools/grep.rs` | 수정 | 정규식 실패 시 리터럴 폴백 |
| `src/agent/prompt.rs` | 수정 | 예시 3개·규칙 2줄 추가 |
| `src/tools/edit_file.rs` | 수정 | 무변경 에러, 성공 컨텍스트, 최근접 인용, multi-match 나열, replace_all |
| `src/agent/repetition.rs` | **신규** | `RepetitionTracker` — (호출,결과 해시) 8턴 윈도 + 동일 에러 연속 감지 |
| `src/agent/mod.rs` | 수정 | salvage 노트 배선, finish 에러 예시, 트래커 통합, 검증 넛지 |
| `docs/baselines.md` | 수정 | M5 경과·최종 결과 절 |
| `CLAUDE.md`, 본선 스펙 | 수정 | 새 동작 반영 (Task 17) |

---

### Task 1: 브랜치 생성 + eval timeout 클램프 (스펙 §4.2)

**Files:**
- Modify: `src/eval/mod.rs` (165행 `Duration::from_secs_f64` 및 222행 check_timeout 산정)

**Interfaces:**
- Produces: `fn scaled_timeout(secs: u64, scale: f64) -> Duration` (eval/mod.rs 내부 함수, 상한 3600초)

- [ ] **Step 1: 브랜치 생성**

```bash
git checkout -b feat/m5-scaffolding
```

- [ ] **Step 2: 실패하는 테스트 작성**

`src/eval/mod.rs`의 기존 테스트 모듈은 `#[cfg(test)] #[cfg(unix)]`로 게이트돼 있다(통합
테스트가 `sh -c` 의존). 이 테스트는 크로스플랫폼이므로 **비게이트 모듈을 신설**해 배치:

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    // Task 1·2의 단위 테스트가 여기 들어간다
}
```

```rust
#[test]
fn scaled_timeout_saturates_and_clamps() {
    use std::time::Duration;
    assert_eq!(scaled_timeout(300, 1.0), Duration::from_secs(300));
    assert_eq!(scaled_timeout(300, 2.0), Duration::from_secs(600));
    // 상한 3600초 — 거대 값·비유한 배율이 from_secs_f64 패닉을 일으키지 않는다 (스펙 §4.2)
    assert_eq!(scaled_timeout(u64::MAX, 1.0), Duration::from_secs(3600));
    assert_eq!(scaled_timeout(300, f64::INFINITY), Duration::from_secs(3600));
    assert_eq!(scaled_timeout(300, f64::NAN), Duration::from_secs(3600));
    assert_eq!(scaled_timeout(300, -1.0), Duration::from_secs(0));
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test scaled_timeout -- --nocapture`
Expected: FAIL — `scaled_timeout` 미정의 컴파일 에러

- [ ] **Step 4: 구현**

`src/eval/mod.rs`에 함수 추가:

```rust
/// timeout × scale — 포화 + 상한 3600초. from_secs_f64는 비유한/음수/오버플로에서
/// 패닉하므로(스펙 M5 §4.2) 유한성 검사 후 클램프한다
fn scaled_timeout(secs: u64, scale: f64) -> Duration {
    const MAX_SECS: f64 = 3600.0;
    let v = secs as f64 * scale;
    let v = if v.is_finite() { v.clamp(0.0, MAX_SECS) } else { MAX_SECS };
    Duration::from_secs_f64(v)
}
```

기존 두 산정처를 교체:

```rust
// run_once (기존 165행):
let limit = scaled_timeout(t.spec.timeout_secs, opts.timeout_scale);
// judge (기존 222행):
let check_timeout = scaled_timeout(t.spec.check_timeout_secs, opts.timeout_scale);
```

- [ ] **Step 5: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/eval/mod.rs
git commit -m "fix(eval): timeout 산정에 포화·상한 3600초 클램프 (M5 Batch 0)"
```

---

### Task 2: `.cargo` 암묵 protected + 상위 경로 트립와이어 (스펙 §4.1)

**Files:**
- Modify: `src/eval/mod.rs` (`judge` 함수)

**Interfaces:**
- Consumes: `Sandbox::sync_protected(&self, fixture: &Path, protected: &[String])` (기존, 무변경 — "fixture에 없으면 삭제" 의미론 재사용)
- Produces: `fn cargo_tripwire(sandbox_root: &Path) -> anyhow::Result<()>` (eval/mod.rs 내부)

- [ ] **Step 1: 실패하는 테스트 작성**

Task 1이 신설한 비게이트 `mod unit_tests`에 추가 (파일시스템만 사용 — 크로스플랫폼):

```rust
#[test]
fn judge_deletes_agent_created_dot_cargo() {
    // sync_protected에 .cargo가 암묵 합류하는지 — judge를 직접 부르지 않고
    // 합집합 헬퍼를 검증한다 (judge는 exec_shell 의존이라 unix 게이트 대상)
    let p = with_implicit_protected(&["tests".to_string()]);
    assert!(p.iter().any(|s| s == ".cargo"));
    assert!(p.iter().any(|s| s == "tests"));
    // 이미 있으면 중복 추가하지 않는다
    let p2 = with_implicit_protected(&[".cargo".to_string()]);
    assert_eq!(p2.iter().filter(|s| *s == ".cargo").count(), 1);
}

#[test]
fn cargo_tripwire_rejects_parent_dot_cargo() {
    let base = tempfile::tempdir().unwrap();
    let sandbox = base.path().join("sb");
    std::fs::create_dir_all(&sandbox).unwrap();
    assert!(cargo_tripwire_from(&sandbox, base.path()).is_ok());
    std::fs::create_dir_all(base.path().join(".cargo")).unwrap();
    let err = cargo_tripwire_from(&sandbox, base.path()).unwrap_err();
    assert!(err.to_string().contains(".cargo"), "{err}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test dot_cargo -- --nocapture` 및 `cargo test cargo_tripwire`
Expected: FAIL — 함수 미정의

- [ ] **Step 3: 구현**

`src/eval/mod.rs`에 추가:

```rust
/// check 판정 자산에 항상 포함되는 암묵 protected — .cargo/config.toml 가짜 러너로
/// 판정을 우회하는 샌드박스 내부 벡터 차단 (스펙 M5 §4.1)
fn with_implicit_protected(protected: &[String]) -> Vec<String> {
    let mut out = protected.to_vec();
    if !out.iter().any(|p| p == ".cargo") {
        out.push(".cargo".to_string());
    }
    out
}

/// 샌드박스 상위 경로(base까지)에 .cargo가 있으면 판정 무결성 훼손으로 하네스 중단.
/// cargo의 config 탐색이 cwd에서 루트로 상향하기 때문 — $CARGO_HOME/홈 벡터는
/// 미차단 잔여 한계 (docs/baselines.md 한계 절 참고)
fn cargo_tripwire_from(sandbox_root: &Path, base: &Path) -> anyhow::Result<()> {
    let mut cur = sandbox_root.parent();
    while let Some(dir) = cur {
        let sus = dir.join(".cargo");
        if sus.exists() {
            anyhow::bail!(
                "판정 무결성 경고: 샌드박스 상위 경로에 .cargo가 있습니다 ({}) — check가 가짜 러너 설정을 읽을 수 있어 중단합니다",
                sus.display()
            );
        }
        if dir == base {
            break;
        }
        cur = dir.parent();
    }
    Ok(())
}

fn cargo_tripwire(sandbox_root: &Path) -> anyhow::Result<()> {
    cargo_tripwire_from(sandbox_root, &std::env::temp_dir())
}
```

`judge()`의 첫 줄을 교체:

```rust
    sb.sync_protected(&t.fixture, &with_implicit_protected(&t.spec.protected))?;
    cargo_tripwire(&sb.root)?;
```

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS. 기존 `src/eval/mod.rs`의 unix 통합 테스트(판정 우회 시나리오 포함)도 통과해야 함 — `.cargo`를 fixture에 두는 테스트는 없으므로 회귀 없음

- [ ] **Step 5: 커밋**

```bash
git add src/eval/mod.rs
git commit -m "fix(eval): .cargo 암묵 protected + 상위 경로 트립와이어 — 판정 우회 차단 (M5 Batch 0)"
```

---

### Task 3: report.json 유효 config 스냅샷 (스펙 §4.3)

**Files:**
- Modify: `src/eval/report.rs` (`Report`에 필드 추가), `src/eval/mod.rs` (`run_eval`에서 채움)

**Interfaces:**
- Produces: `pub struct EffectiveConfig { pub base_url: String, pub temperature: f32, pub context_tokens: usize, pub max_output_tokens: usize, pub max_turns: usize, pub command_timeout_secs: u64, pub loco_version: String }` + `Report.effective_config: EffectiveConfig`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/eval/report.rs` tests — `sample_report()`가 컴파일되도록 고치는 것 자체가 구현의 일부. 우선 단언만 추가:

```rust
#[test]
fn report_json_snapshots_effective_config() {
    let v = serde_json::to_value(sample_report()).unwrap();
    let ec = v.get("effective_config").expect("유효 config 스냅샷 (스펙 M5 §4.3)");
    for key in ["base_url", "temperature", "context_tokens", "max_output_tokens", "max_turns", "command_timeout_secs", "loco_version"] {
        assert!(ec.get(key).is_some(), "effective_config에 {key}");
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test snapshots_effective_config`
Expected: FAIL — 필드 없음(컴파일 에러 또는 단언 실패)

- [ ] **Step 3: 구현**

`src/eval/report.rs`:

```rust
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
```

`Report`에 `pub effective_config: EffectiveConfig,` 필드 추가. `sample_report()`에 채움:

```rust
            effective_config: EffectiveConfig {
                base_url: "http://localhost:1234/v1".into(),
                temperature: 0.1,
                context_tokens: 8192,
                max_output_tokens: 2048,
                max_turns: 25,
                command_timeout_secs: 60,
                loco_version: "test".into(),
            },
```

`src/eval/mod.rs`의 `run_eval`에서 Report 생성부에 추가 (`use report::EffectiveConfig` 포함):

```rust
        effective_config: EffectiveConfig {
            base_url: config.base_url.clone(),
            temperature: config.temperature,
            context_tokens: config.context_tokens,
            max_output_tokens: config.max_output_tokens,
            max_turns: config.max_turns,
            command_timeout_secs: config.command_timeout_secs,
            loco_version: env!("CARGO_PKG_VERSION").to_string(),
        },
```

주의: `run_once`는 과제별 `max_turns` 오버라이드를 하므로 스냅샷은 **전역 config 기준**임을 필드 주석에 명시하지 않아도 된다 — 과제별 오버라이드는 task.toml에 이미 기록돼 있다.

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (report.rs의 기존 스키마 테스트 포함)

- [ ] **Step 5: 커밋**

```bash
git add src/eval/report.rs src/eval/mod.rs
git commit -m "feat(eval): report.json에 유효 config 스냅샷 기록 (M5 Batch 0)"
```

---

### Task 4: protocol salvage 파싱 (스펙 §5.1)

**Files:**
- Modify: `src/agent/protocol.rs` (parse_turn을 Value 경유로 재구성), `src/agent/mod.rs` (salvage 노트 배선)

**Interfaces:**
- Produces: `ModelTurn { pub thought: String, pub action: Action, pub salvaged: bool }` — `salvaged`는 정규화 발동 표식. `Action { pub tool: String, pub args: serde_json::Value }` 유지. `pub const SALVAGE_NOTE: &str` (agent/mod.rs)
- 계약 유지: args 부재·null이고 salvage 미발동이면 `args == Value::Null` (기존 테스트 `missing_args_defaults_to_null`)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/protocol.rs` tests에 추가 — 전부 기준선 트랜스크립트 실측 형태:

```rust
#[test]
fn salvages_action_level_scalar_fields_into_args() {
    // qwen fix-compile-error 실측: 인자를 action 레벨에 둠
    let t = parse_turn(r#"{"thought": "build", "action": {"args": {}, "tool": "run_command", "command": "cargo build"}}"#).unwrap();
    assert_eq!(t.action.args["command"], "cargo build");
    assert!(t.salvaged);
}

#[test]
fn salvages_args_2_object_overwriting_stale_args() {
    // gemma add-function 실측: args에 grep 잔재, 진짜 인자는 args_2
    let t = parse_turn(r#"{"thought": "edit", "action": {"args": {"pattern": "median", "path": "src"}, "tool": "edit_file", "args_2": {"search": "todo!()", "replace": "42", "path": "src/lib.rs"}}}"#).unwrap();
    assert_eq!(t.action.args["search"], "todo!()");
    assert_eq!(t.action.args["path"], "src/lib.rs", "args_2(키 이름 뒤 순서)가 stale args를 덮는다");
    assert_eq!(t.action.args["pattern"], "median", "잔재 키는 남아도 무해 — 툴이 무시");
    assert!(t.salvaged);
}

#[test]
fn salvages_finish_summary_from_args_2() {
    let t = parse_turn(r#"{"thought": "done", "action": {"tool": "finish", "args_2": {"summary": "답"}}}"#).unwrap();
    assert_eq!(t.action.args["summary"], "답");
    assert!(t.salvaged);
}

#[test]
fn top_level_unknown_scalar_is_promoted_but_reserved_names_are_not() {
    let t = parse_turn(r#"{"thought": "run", "action": {"tool": "run_command"}, "command": "ls", "args": {"junk": 1}}"#).unwrap();
    assert_eq!(t.action.args["command"], "ls");
    assert!(t.action.args.get("junk").is_none(), "최상위 tool/args*는 예약어 — 승격 금지 (플랫 턴은 범위 제외)");
}

#[test]
fn scalar_promotion_does_not_overwrite_existing_args() {
    let t = parse_turn(r#"{"thought": "r", "action": {"tool": "read_file", "args": {"path": "good.rs"}, "path": "junk"}}"#).unwrap();
    assert_eq!(t.action.args["path"], "good.rs", "스칼라 승격은 부재 시 삽입만");
}

#[test]
fn non_object_args_pass_through_without_salvage() {
    let t = parse_turn(r#"{"thought": "x", "action": {"tool": "grep", "args": "fn main"}}"#).unwrap();
    assert_eq!(t.action.args, serde_json::json!("fn main"));
    assert!(!t.salvaged);
}

#[test]
fn clean_turns_are_not_marked_salvaged() {
    let t = parse_turn(r#"{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}"#).unwrap();
    assert!(!t.salvaged);
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test protocol`
Expected: FAIL — `salvaged` 필드 없음(컴파일 에러)

- [ ] **Step 3: 구현**

`src/agent/protocol.rs` — `ModelTurn`/`Action`에서 `Deserialize` derive 제거(수동 구성으로 전환), `salvaged` 추가:

```rust
/// 매 턴 모델이 출력해야 하는 구조 (스펙 §4)
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTurn {
    pub thought: String,
    pub action: Action,
    /// salvage 정규화(M5 §5.1)가 적용됐는지 — 루프가 툴 결과에 교정 노트를 붙인다
    pub salvaged: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub tool: String,
    pub args: serde_json::Value,
}
```

`parse_turn`의 사다리 3단을 전부 `serde_json::Value` 파싱 + `build_turn` 호출로 교체:

```rust
pub fn parse_turn(text: &str) -> Result<ModelTurn, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("Your reply was empty. {FORMAT_HINT}"));
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return build_turn(v);
    }
    if let Some(inner) = strip_fence(text)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(inner)
    {
        return build_turn(v);
    }
    if let Some(obj) = first_json_object(text) {
        return match serde_json::from_str::<serde_json::Value>(obj) {
            Ok(v) => build_turn(v),
            Err(e) => Err(format!("Your reply was not a valid turn ({e}). {FORMAT_HINT}")),
        };
    }
    Err(format!("Your reply contained no JSON object. {FORMAT_HINT}"))
}
```

(`FORMAT_HINT`는 함수 밖 모듈 상수로 승격: `const FORMAT_HINT: &str = r#"Reply with exactly one JSON object: {"thought": "...", "action": {"tool": "...", "args": {...}}}"#;`)

`build_turn` 신규:

```rust
/// Value → ModelTurn + salvage 정규화 (M5 §5.1). serde_json 기본 Map은 BTreeMap이라
/// 순회가 키 이름 오름차순 — 병합이 결정론적이고 나중 키(args_2 등)가 이긴다.
/// 스칼라 승격은 부재 시 삽입만(기존 args 보호), 오브젝트 병합은 덮어쓴다(최신 의도)
fn build_turn(v: serde_json::Value) -> Result<ModelTurn, String> {
    let serde_json::Value::Object(mut top) = v else {
        return Err(format!("Your reply was not a valid turn (not a JSON object). {FORMAT_HINT}"));
    };
    let thought = match top.remove("thought") {
        Some(serde_json::Value::String(s)) => s,
        _ => return Err(format!("Your reply was not a valid turn (missing field `thought`). {FORMAT_HINT}")),
    };
    let serde_json::Value::Object(mut act) = top.remove("action").unwrap_or(serde_json::Value::Null) else {
        return Err(format!("Your reply was not a valid turn (missing field `action`). {FORMAT_HINT}"));
    };
    let tool = match act.remove("tool") {
        Some(serde_json::Value::String(s)) => s,
        _ => return Err(format!("Your reply was not a valid turn (missing field `tool` in action). {FORMAT_HINT}")),
    };
    let (mut map, args_was_object) = match act.remove("args") {
        Some(serde_json::Value::Object(m)) => (m, true),
        Some(serde_json::Value::Null) | None => (serde_json::Map::new(), false),
        // 비오브젝트 args는 salvage 불가 — 그대로 전달, 툴 쪽 BadArgs가 처리
        Some(other) => return Ok(ModelTurn { thought, action: Action { tool, args: other }, salvaged: false }),
    };
    let mut salvaged = false;
    for (k, val) in act {
        salvaged |= merge_entry(&mut map, k, val);
    }
    for (k, val) in top {
        if k == "tool" || k.starts_with("args") {
            continue; // 예약어 — 플랫 턴 변형과의 혼동 방지 (M5 §5.1 범위 제외)
        }
        salvaged |= merge_entry(&mut map, k, val);
    }
    let args = if map.is_empty() && !args_was_object {
        serde_json::Value::Null // 기존 계약: args 부재 → Null
    } else {
        serde_json::Value::Object(map)
    };
    Ok(ModelTurn { thought, action: Action { tool, args }, salvaged })
}

/// k/val을 args 맵에 병합. 오브젝트는 엔트리 덮어쓰기(최신 의도 우선), 스칼라는
/// 부재 시에만 삽입(기존 args 보호). 반환: 실제로 뭔가 넣었는지
fn merge_entry(map: &mut serde_json::Map<String, serde_json::Value>, k: String, val: serde_json::Value) -> bool {
    match val {
        serde_json::Value::Object(inner) => {
            let mut any = false;
            for (ik, iv) in inner {
                map.insert(ik, iv);
                any = true;
            }
            any
        }
        other => {
            if map.contains_key(&k) {
                false
            } else {
                map.insert(k, other);
                true
            }
        }
    }
}
```

주의: `Deserialize` derive 제거 후 파일 상단의 `use serde::Deserialize;`가 미사용 import가
된다 — 삭제할 것 (-D warnings 게이트).

- [ ] **Step 4: 프로토콜 테스트 통과 확인**

Run: `cargo test protocol`
Expected: 신규 7개 + 기존 8개 전부 PASS. 특히 `missing_args_defaults_to_null`(Null 유지), `valid_json_with_wrong_shape_is_an_error`("thought" 언급) 무변경 통과

- [ ] **Step 5: salvage 노트 배선 — 실패하는 테스트**

`src/agent/mod.rs` tests에 추가:

```rust
#[tokio::test]
async fn salvaged_turn_gets_a_note_with_the_tool_result() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
    // path를 action 레벨에 둔 salvage 대상 턴
    let bad_shape = r#"{"thought": "read", "action": {"tool": "read_file", "args": {}, "path": "a.txt"}}"#;
    let script = Scripted::new(vec![ok(bad_shape), ok(&finish("done"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    let note = session.messages().iter().find(|m| m.content.contains("fields outside"));
    let note = note.expect("salvage 노트가 툴 결과에 병합");
    assert_eq!(note.role, "user");
    assert!(note.content.contains("hi"), "툴 결과(파일 내용)와 같은 메시지: {}", note.content);
}
```

- [ ] **Step 6: 배선 구현**

`src/agent/mod.rs`에 상수 추가:

```rust
/// salvage 발동 시 툴 결과에 붙이는 교정 노트 (M5 §5.1). 모델 대상 — 영어
pub const SALVAGE_NOTE: &str =
    "note: fields outside \"args\" were accepted this time - put them inside \"args\".";
```

루프의 두 `push_tool_result` 지점(거부 branch, 디스패치 branch)에서 note 조립을 다음 패턴으로 교체 — 기존 `REPEAT_CORRECTION` 로직 유지, salvage 노트를 병합:

```rust
            let mut notes: Vec<&str> = Vec::new();
            if turn.salvaged {
                notes.push(SALVAGE_NOTE);
            }
            if repeat_count == 3 && !corrected {
                corrected = true;
                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                notes.push(REPEAT_CORRECTION);
            }
            let joined = notes.join("\n");
            let note = (!joined.is_empty()).then_some(joined.as_str());
            session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note);
```

(거부 branch에도 동일 패턴 — 기존 `Some(REPEAT_CORRECTION)`/`None` 조립을 대체. Task 14에서 이 영역이 다시 바뀌므로 여기서는 최소 변경)

- [ ] **Step 7: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS

- [ ] **Step 8: 커밋**

```bash
git add src/agent/protocol.rs src/agent/mod.rs
git commit -m "feat(agent): 툴 인자 salvage 파싱 — args 밖 필드·args_2 승격 (M5 Batch 1)"
```

---

### Task 5: dispatch 스키마 에코 + finish 에러 형태 예시 (스펙 §5.2)

**Files:**
- Modify: `src/tools/mod.rs` (`Registry::dispatch`), `src/agent/mod.rs` (finish 에러 문구)

**Interfaces:**
- Consumes: `Tool::doc()` — 각 툴의 시그니처 문자열 (기존)
- Produces: dispatch가 serde 계열 BadArgs를 `"{원본}. Expected: {doc}. You sent keys: [{keys}]."`로 확장

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/mod.rs` tests에 추가:

```rust
#[test]
fn serde_bad_args_echoes_schema_and_received_keys() {
    let reg = Registry::guided();
    let err = reg
        .dispatch("edit_file", &serde_json::json!({"pattern": "median", "path": "src"}), &ctx())
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing field"), "{msg}");
    assert!(msg.contains("edit_file(path, search, replace"), "기대 시그니처 에코: {msg}");
    assert!(msg.contains("pattern") && msg.contains("path"), "수신 키 목록: {msg}");
}

#[test]
fn semantic_bad_args_are_not_wrapped() {
    // read_file의 offset 초과는 이미 구체적 — 스키마 에코를 붙이지 않는다
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "one line").unwrap();
    let reg = Registry::guided();
    let c = ToolCtx::new(dir.path().to_path_buf());
    let err = reg.dispatch("read_file", &serde_json::json!({"path": "f.txt", "offset": 99}), &c).unwrap_err();
    assert!(!err.to_string().contains("Expected:"), "{err}");
}
```

(tools/mod.rs 테스트 모듈에 `tempfile` 사용이 처음이면 기존 다른 툴 테스트와 동일하게 dev-dependency로 이미 존재)

- [ ] **Step 2: 실패 확인**

Run: `cargo test echoes_schema`
Expected: FAIL — 에코 없음

- [ ] **Step 3: 구현**

`Registry::dispatch`의 `tool.run(args, ctx)` 반환을 래핑:

```rust
        tool.run(args, ctx).map_err(|e| match e {
            // serde 인자 실패에만 스키마 에코 (M5 §5.2) — 의미 오류(빈 search, offset 초과
            // 등)는 이미 구체적이라 소음만 된다. serde 메시지 감지는 문구 기반(취약하지만
            // serde_json 에러 타입을 구분할 다른 방법이 없음)
            ToolError::BadArgs(msg) if msg.contains("missing field") || msg.contains("invalid type") => {
                let keys = args
                    .as_object()
                    .map(|m| m.keys().cloned().collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                ToolError::BadArgs(format!("{msg}. Expected: {}. You sent keys: [{keys}].", tool.doc()))
            }
            other => other,
        })
```

`src/agent/mod.rs`의 finish 에러 문구(기존 236행)를 교체:

```rust
                            "Error: finish requires a string `summary` argument, e.g. {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}",
```

기존 테스트 `finish_without_summary_gets_feedback`가 문구를 검사한다면 새 문구에 맞게 단언 갱신 (`requires a string` 부분 문자열은 유지되므로 대부분 무변경 통과).

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS

- [ ] **Step 5: 커밋**

```bash
git add src/tools/mod.rs src/agent/mod.rs
git commit -m "feat(tools): serde 인자 에러에 기대 시그니처·수신 키 에코 (M5 Batch 1)"
```

---

### Task 6: read_file 디렉터리 번역 + grep 리터럴 폴백 (스펙 §5.3)

**Files:**
- Modify: `src/tools/read_file.rs`, `src/tools/grep.rs`

**Interfaces:**
- Produces: read_file — 디렉터리 경로에 `BadArgs("<path> is a directory, not a file - use list_files for directories")`. grep — 정규식 파싱 실패 시 리터럴 검색으로 폴백, 출력 헤더 `invalid regex (<reason>); searched for the literal text instead - N matches`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/read_file.rs` tests:

```rust
#[test]
fn directory_path_gets_a_helpful_error_not_os_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let ctx = ToolCtx::new(dir.path().to_path_buf());
    let err = run(&ctx, serde_json::json!({"path": "src"})).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("is a directory"), "{msg}");
    assert!(msg.contains("list_files"), "대안 안내: {msg}");
    assert!(!msg.contains("os error"), "날 OS 에러 노출 금지: {msg}");
}
```

`src/tools/grep.rs` tests:

```rust
#[test]
fn invalid_regex_falls_back_to_literal_search() {
    // gemma multiline-string-edit 실측: 리터럴 {user_name}가 정규식 파싱 실패
    let (_d, ctx) = setup();
    std::fs::write(_d.path().join("src/c.rs"), "user: {user_name}\n").unwrap();
    let out = Grep.run(&serde_json::json!({"pattern": "{user_name}"}), &ctx).unwrap();
    assert!(out.contains("invalid regex"), "{out}");
    assert!(out.contains("literal"), "{out}");
    assert!(out.contains("c.rs"), "리터럴 매치 발견: {out}");
}

#[test]
fn invalid_regex_with_zero_matches_still_reports_the_fallback() {
    let (_d, ctx) = setup();
    let out = Grep.run(&serde_json::json!({"pattern": "{no_such_thing}"}), &ctx).unwrap();
    assert!(out.contains("invalid regex"), "{out}");
    assert!(out.contains("0 matches"), "0매치도 원인 병기 (스펙 §5.3): {out}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test directory_path_gets && cargo test invalid_regex`
Expected: FAIL

- [ ] **Step 3: read_file 구현**

`run()`의 `let path = confine(...)?;` 직후에:

```rust
        if path.is_dir() {
            return Err(ToolError::BadArgs(format!(
                "{} is a directory, not a file - use list_files for directories",
                args.path
            )));
        }
```

- [ ] **Step 4: grep 구현**

`run()`의 정규식 컴파일부를 교체:

```rust
        let (re, fallback_reason) = match regex::Regex::new(&args.pattern) {
            Ok(re) => (re, None),
            Err(e) => {
                // 코드 조각({user_name} 등)이 정규식 파싱에 실패하면 리터럴로 폴백 (M5 §5.3)
                let literal = regex::Regex::new(&regex::escape(&args.pattern))
                    .map_err(|e2| ToolError::BadArgs(format!("invalid regex: {e2}")))?;
                let reason: String = e.to_string().split_whitespace().collect::<Vec<_>>().join(" ");
                let reason = if reason.len() > 120 { format!("{}...", &reason[..120]) } else { reason };
                (literal, Some(reason))
            }
        };
```

(주의: `&reason[..120]`은 char 경계 패닉 가능 — regex 에러 메시지는 ASCII지만 방어적으로 `reason.chars().take(120).collect::<String>()` 사용)

함수 말미의 반환부를 교체:

```rust
        let body = if matches == 0 { String::new() } else { out.trim_end().to_string() };
        if let Some(reason) = fallback_reason {
            let header = format!("invalid regex ({reason}); searched for the literal text instead - {matches} matches");
            let mut res = if body.is_empty() { header } else { format!("{header}\n{body}") };
            if truncated {
                res.push_str(&format!("\n[more matches truncated at {MAX_MATCHES}]"));
            }
            return Ok(res);
        }
        if matches == 0 {
            return Ok("no matches".to_string());
        }
        if truncated {
            return Ok(format!("{body}\n[more matches truncated at {MAX_MATCHES}]"));
        }
        Ok(body)
```

(기존 truncated 처리가 out에 이미 붙는 구조면 그 구조를 유지하되 fallback 헤더만 앞에 붙이는 최소 변경도 가능 — 최종 형태는 기존 테스트가 전부 통과해야 한다)

- [ ] **Step 5: 기존 테스트 갱신 + 통과 확인 + 게이트**

기존 `invalid_regex_is_bad_args` 테스트(src/tools/grep.rs:137-142 부근)는 폴백 도입으로
의미가 뒤집힌다(`"["`가 이제 성공) — 다음으로 교체:

```rust
    #[test]
    fn invalid_regex_is_no_longer_an_error_but_a_literal_fallback() {
        let (_d, ctx) = setup();
        let out = Grep.run(&serde_json::json!({"pattern": "["}), &ctx).unwrap();
        assert!(out.starts_with("invalid regex"), "{out}");
        assert!(out.contains("literal"), "{out}");
    }
```

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (grep의 나머지 기존 테스트 포함)

- [ ] **Step 6: 커밋**

```bash
git add src/tools/read_file.rs src/tools/grep.rs
git commit -m "feat(tools): 디렉터리 에러 번역 + grep 리터럴 폴백 (M5 Batch 1)"
```

---

### Task 7: 시스템 프롬프트 개정 (스펙 §5.4)

**Files:**
- Modify: `src/agent/prompt.rs`

**Interfaces:**
- Consumes: 없음 (문자열만). 프롬프트는 ASCII 유지 (기존 테스트 `p.is_ascii()`)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/prompt.rs`의 기존 `prompt_states_protocol_and_finish_channel` 테스트에 단언 추가:

```rust
        // M5 §5.4: 검증 규칙 + 정확 복사 규칙 + mutating 포함 예시 3개
        assert!(p.contains("verify with run_command"), "검증 규칙");
        assert!(p.contains("Copy `search` text exactly"), "정확 복사 규칙");
        assert!(p.contains("\"tool\": \"edit_file\""), "edit_file 예시");
        assert!(p.contains("\"tool\": \"run_command\""), "run_command 예시");
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test prompt_states`
Expected: FAIL

- [ ] **Step 3: 구현**

`system_prompt`의 format! 문자열에서 Rules와 Example 절을 교체 (전체 교체본 — `{{`는 format! 이스케이프):

```rust
    format!(
        "You are loco, a coding agent working inside the user's project directory. \
You interact with the project ONLY by calling tools.\n\
\n\
Respond with exactly ONE JSON object per turn and nothing else:\n\
{{\"thought\": \"<one short sentence of reasoning, in English>\", \"action\": {{\"tool\": \"<name>\", \"args\": {{...}}}}}}\n\
\n\
Rules:\n\
- One tool call per turn. All tool parameters go inside \"args\".\n\
- Never repeat a tool call that already returned a result - reuse that result. As soon as you have enough information, call `finish`.\n\
- To change an existing file, prefer `edit_file` with a small unique search block. Copy `search` text exactly from the latest read_file output. Use `write_file` only for new files or full rewrites.\n\
- After changing files, verify with run_command (e.g. `cargo test`) before finish.\n\
- File paths are relative to the project root. Explore with list_files or grep before reading whole files.\n\
- When you know the answer (or cannot proceed), call `finish`. Its `summary` is the ONLY text shown to the user - put the complete answer there, written in the user's language.\n\
\n\
Tools:\n\
{tool_docs}\n\
- finish(summary): End the task and give `summary` to the user as the final answer.\n\
\n\
Example turns:\n\
{{\"thought\": \"I need to find where the config is loaded.\", \"action\": {{\"tool\": \"grep\", \"args\": {{\"pattern\": \"fn load\", \"path\": \"src\"}}}}}}\n\
{{\"thought\": \"Replace the todo with the real body.\", \"action\": {{\"tool\": \"edit_file\", \"args\": {{\"path\": \"src/lib.rs\", \"search\": \"fn add(a: i32, b: i32) -> i32 {{\\n    todo!()\\n}}\", \"replace\": \"fn add(a: i32, b: i32) -> i32 {{\\n    a + b\\n}}\"}}}}}}\n\
{{\"thought\": \"Verify my edit compiles and tests pass.\", \"action\": {{\"tool\": \"run_command\", \"args\": {{\"command\": \"cargo test\"}}}}}}\n\
\n\
Project files (partial, gitignore respected):\n\
{tree}"
    )
```

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (`is_ascii` 포함 — 예시의 `\\n`은 리터럴 백슬래시+n으로 ASCII).
`agent::` 테스트 중 `every_turn_packs_to_budget`(src/agent/mod.rs:729 부근)를 특히 확인 —
프롬프트가 ~500바이트 커져 여유가 ~100토큰으로 얇아진다. 실패하면 그 테스트의 주석
지침대로 수치를 재조정(테스트 파일 크기 상향)하고 커밋 메시지에 명기

- [ ] **Step 5: 커밋**

```bash
git add src/agent/prompt.rs
git commit -m "feat(agent): 시스템 프롬프트에 mutating 예시·검증 규칙 추가 (M5 Batch 1)"
```

---

### Task 8: [체크포인트 — 사용자 협조] Batch 1 qwen 측정

**Files:**
- Modify: `docs/baselines.md` (M5 경과 절 신설)

측정 조건은 기준선과 동일해야 한다: LM Studio에 qwen3-vl-4b **단독** 로드(ctx 8192), 저장소 로컬 `./.loco/config.toml`에 `max_output_tokens = 4096`.

- [ ] **Step 1: 사용자에게 측정 준비 요청**

사용자에게 알림: "Batch 1 측정 준비 — LM Studio에 qwen3-vl-4b를 ctx 8192로 단독 로드해 주세요." 트립와이어 사전 점검(Task 2가 추가 — 걸리면 하네스가 전 과제 중단):

```bash
ls "${TMPDIR:-/tmp}/.cargo" 2>/dev/null && echo "경고: temp_dir/.cargo 존재 — 제거 필요" || echo OK
```

로드 상태 확인 방법:

```bash
curl -s localhost:1234/api/v0/models | grep -o '"id":"[^"]*"\|"loaded_context_length":[0-9]*'
```

- [ ] **Step 2: 측정 실행 (~40-75분)**

```bash
cargo build --release
./target/release/loco eval tasks --repeats 3
```

Expected: 종료 코드 0, `.loco/eval/<새 스탬프>/`에 report.json + 36개 트랜스크립트

- [ ] **Step 3: 메커니즘 지표 집계**

```bash
S=.loco/eval/<새 스탬프>
grep -o 'missing field' $S/run-*.jsonl | wc -l          # ① 기준선(qwen): 다수 — 감소 기대
grep -o 'exit code:' $S/run-*.jsonl | wc -l             # ② run_command 실행 수 — 증가 기대
grep -o 'search block not found' $S/run-*.jsonl | wc -l # ③ 참고(Batch 2 대상)
grep -o '"outcome": "[a-z_]*"' $S/report.json | sort | uniq -c  # ④⑤ 분포
grep -A1 '"passed": false' $S/report.json | grep -c '"outcome": "finished"'  # 거짓 성공 finish 수 (② 핵심 지표 — RunRecord 직렬화가 passed 다음 줄에 outcome)
```

기준선 대조용 동일 집계를 `.loco/eval/20260711T235558Z`(qwen 기준선)에도 실행해 비교.

- [ ] **Step 4: keep/revert 판정 (스펙 §3)**

- 전체 통과 수 vs 기준선 12/36: **-2런 이상 + 지표 악화 동반**이거나 확인 재측정에서 재현될 때만 원인 항목 revert. ±1런은 keep
- 판정 근거(통과 수·지표 수치)를 기록

- [ ] **Step 5: 결과 기록 + 커밋**

`docs/baselines.md` 말미에 절 추가 (이후 배치가 행을 덧붙임):

```markdown
## M5 경과 (배치별 qwen 측정)

| 배치 | 통과 | missing field | run_command 실행 | not found | outcome 분포 | 판정 |
|---|---|---|---|---|---|---|
| 기준선 | 12/36 | (집계) | (집계) | (집계) | F12/M17/R7 | — |
| Batch 1 | ?/36 | ? | ? | ? | ? | keep/revert |
```

```bash
git add docs/baselines.md
git commit -m "docs: M5 Batch 1 qwen 측정 결과"
```

---

### Task 9: edit_file — 무변경 에러 + 성공 컨텍스트 (스펙 §6.1·§6.5)

**Files:**
- Modify: `src/tools/edit_file.rs`

**Interfaces:**
- Produces: `apply_edit(text, search, replace) -> Result<EditOutcome, String>` where `struct EditOutcome { new_text: String, mode: MatchMode, start_line: usize, replaced_lines: usize, occurrences: usize }` (start_line은 새 텍스트 기준 0-기준; Task 11의 replace_all이 occurrences를 2+로 만든다, 이 태스크에서는 항상 1)
- run() 성공 메시지: `"Edited {path} (matched {mode})\nContext after edit (lines {A}-{B}):\n{±3줄}\nVerify this is what you intended."`
- preview()는 기존 diff 유지 (확인 게이트 UI 불변)

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[test]
fn identical_search_and_replace_is_an_error() {
    let (_d, ctx) = setup("fn a() {}\n");
    let err = edit(&ctx, "fn a() {}", "fn a() {}").unwrap_err();
    assert!(err.to_string().contains("identical"), "{err}");
}

#[test]
fn success_reports_post_edit_context_with_line_numbers_in_header_only() {
    let (_d, ctx) = setup("l1\nl2\nl3\nl4\nOLD\nl6\nl7\nl8\nl9\n");
    let out = edit(&ctx, "OLD", "NEW").unwrap();
    assert!(out.contains("Context after edit (lines 2-8):"), "{out}");
    assert!(out.contains("NEW"), "{out}");
    assert!(out.contains("l2\nl3\nl4\nNEW\nl6\nl7\nl8"), "±3줄 원문 — 줄번호 접두 금지: {out}");
    assert!(out.contains("Verify this is what you intended"), "{out}");
}

#[test]
fn context_clamps_at_file_boundaries() {
    let (_d, ctx) = setup("OLD\nl2\n");
    let out = edit(&ctx, "OLD", "NEW").unwrap();
    assert!(out.contains("Context after edit (lines 1-2):"), "{out}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test edit_file`
Expected: 신규 3개 FAIL

- [ ] **Step 3: 구현**

`apply_edit`의 반환형을 `EditOutcome`으로 재구성:

```rust
struct EditOutcome {
    new_text: String,
    mode: MatchMode,
    /// 새 텍스트 기준 치환 시작 줄(0-기준) — 성공 컨텍스트 렌더링용 (첫 매치)
    start_line: usize,
    /// 치환으로 들어간 줄 수 (최소 1)
    replaced_lines: usize,
    occurrences: usize,
}
```

1단계(정확 일치)에서 위치 계산:

```rust
    let exact_positions: Vec<usize> = text.match_indices(search).map(|(i, _)| i).collect();
    match exact_positions.len() {
        1 => {
            let start_line = text[..exact_positions[0]].matches('\n').count();
            let replaced_lines = replace.split('\n').count().max(1);
            return Ok(EditOutcome {
                new_text: text.replacen(search, replace, 1),
                mode: MatchMode::Exact,
                start_line,
                replaced_lines,
                occurrences: 1,
            });
        }
        n if n >= 2 => { /* 기존 모호 에러 (Task 10에서 개선) */ }
        _ => {}
    }
```

2·3단계는 splice 위치 `i`와 `replacement.len()`을 그대로 채운다 (occurrences: 1).

`dry_run` 반환을 `(String, EditOutcome, bool /*crlf*/)` 형태로 조정하고, `run()`:

```rust
    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (_old, outcome, crlf) = self.dry_run(&args, ctx)?;
        let path = confine(&ctx.root, &args.path)?;
        std::fs::write(&path, restore_eol(&outcome.new_text, crlf))?;
        // occurrences 분기를 지금부터 사용 — Task 11 전까지는 항상 1이지만, 안 읽는
        // private 필드는 dead_code로 -D warnings 게이트를 깨뜨린다
        let head = if outcome.occurrences > 1 {
            format!("Edited {} (replaced {} occurrences, matched {})", args.path, outcome.occurrences, outcome.mode.describe())
        } else {
            format!("Edited {} (matched {})", args.path, outcome.mode.describe())
        };
        Ok(format!("{head}\n{}", render_context(&outcome.new_text, outcome.start_line, outcome.replaced_lines)))
    }
```

`preview()`도 dry_run 반환형 변경에 맞춰 갱신 (diff 형식은 기존 유지):

```rust
    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (old, outcome, _crlf) = self.dry_run(&args, ctx)?;
        Ok(format!("edit_file {} ({})\n{}", args.path, outcome.mode.describe(), render_diff(&old, &outcome.new_text)))
    }
```

```rust
/// 편집 후 변경 부위 ±3줄 (M5 §6.1). 줄번호는 헤더에만 — 본문에 접두를 붙이면
/// 모델이 다음 search에 복사한다
fn render_context(new_text: &str, start_line: usize, replaced_lines: usize) -> String {
    let mut lines: Vec<&str> = new_text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop(); // 후행 개행의 빈 꼬리 줄은 컨텍스트·줄 범위에서 제외
    }
    let from = start_line.saturating_sub(3);
    let to = (start_line + replaced_lines + 3).min(lines.len());
    format!(
        "Context after edit (lines {}-{}):\n{}\nVerify this is what you intended.",
        from + 1,
        to,
        lines[from..to].join("\n")
    )
}
```

무변경 에러는 `dry_run`에서 정규화 직후:

```rust
        if search == replace {
            return Err(ToolError::EditFailed(
                "search and replace are identical - no change would be made".to_string(),
            ));
        }
```

- [ ] **Step 4: 기존 테스트 갱신 및 통과 확인**

기존 8개 중 성공 메시지를 검사하는 테스트(`exact_match_replaces_once_and_reports_mode` 등)는 `out.contains("exact")` 같은 부분 검사라 대부분 무변경 통과. `preview_is_a_dry_run_diff_without_writing`은 diff 유지로 무변경. 실패가 있으면 새 메시지 형식에 맞게 단언만 갱신.

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS

- [ ] **Step 5: 커밋**

```bash
git add src/tools/edit_file.rs
git commit -m "feat(tools): edit_file 성공 시 결과 컨텍스트 반환 + 무변경 편집 에러 (M5 Batch 2)"
```

---

### Task 10: edit_file — not-found 최근접 인용 + multi-match 위치 나열 (스펙 §6.2·§6.3)

**Files:**
- Modify: `src/tools/edit_file.rs`

**Interfaces:**
- Consumes: Task 9의 `EditOutcome` 구조
- Produces: not-found 에러 `"search block not found. Closest match at lines {A}-{B}:\n{실제 텍스트, 최대 10줄}\nCopy this text exactly into `search` if this is the location you meant."`; 모호 에러 `"search block matches {n} locations ({stage}):\n  line {i}: {첫 줄}...(최대 5개)\nadd surrounding lines to pick one, or set \"replace_all\": true if you intend to change every occurrence"`

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[test]
fn not_found_quotes_the_closest_actual_text() {
    // 이스케이프 깊이 불일치 시나리오: 모델의 search가 실제와 한 글자 다름
    let (_d, ctx) = setup("fn top() {}\n    t.push_str(\"said: \\\"hi\\\"\");\nfn bot() {}\n");
    let err = edit(&ctx, "t.push_str(\"said: \"hi\"\");", "x").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Closest match at lines 2-2:"), "{msg}");
    assert!(msg.contains("t.push_str(\"said: \\\"hi\\\"\");"), "실제 파일 원문 인용: {msg}");
    assert!(msg.contains("Copy this text exactly"), "{msg}");
}

#[test]
fn not_found_quote_is_capped_at_ten_lines() {
    let body: String = (1..=30).map(|i| format!("line{i}\n")).collect();
    let (_d, ctx) = setup(&body);
    let search: String = (1..=20).map(|i| format!("line{i}X\n")).collect(); // 20줄, 첫 줄에서 부분 매치
    let err = edit(&ctx, &search, "x").unwrap_err();
    let quoted = err.to_string().matches("line").count();
    assert!(quoted <= 12, "인용 최대 10줄 (스펙 §6.2 크기 상한): {err}");
}

#[test]
fn multi_match_lists_line_numbers_and_suggests_replace_all() {
    let (_d, ctx) = setup("dup();\nmid\ndup();\nmid\ndup();\n");
    let err = edit(&ctx, "dup();", "x();").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("matches 3 locations"), "{msg}");
    assert!(msg.contains("line 1") && msg.contains("line 3") && msg.contains("line 5"), "{msg}");
    assert!(msg.contains("replace_all"), "{msg}");
}

#[test]
fn multi_match_listing_is_capped_at_five() {
    let body = "dup();\n".repeat(9);
    let (_d, ctx) = setup(&body);
    let err = edit(&ctx, "dup();", "x();").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("matches 9 locations"), "{msg}");
    assert!(msg.contains("and 4 more"), "5개 초과는 생략 표기 (스펙 §6.3): {msg}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test edit_file`
Expected: 신규 4개 FAIL (기존 `two_exact_matches_is_an_immediate_ambiguity_error`·`not_found_reports_near_miss_line`은 이 태스크에서 새 메시지에 맞게 갱신)

- [ ] **Step 3: 구현 — not-found**

`not_found_message`를 교체:

```rust
/// not-found에 최근접 실제 텍스트를 인용 (M5 §6.2) — 모델이 복사만 하면 되게.
/// 탐색: search 첫 줄 부분 매치 → 없으면 문자 bigram 중첩 최대 라인(임계 0.25)
fn not_found_message(text: &str, s_lines: &[&str]) -> String {
    const MAX_QUOTE: usize = 10;
    let first = s_lines.first().map(|l| l.trim()).unwrap_or("");
    let lines: Vec<&str> = text.split('\n').collect();
    let found = if first.is_empty() {
        None
    } else {
        lines.iter().position(|l| l.contains(first)).or_else(|| best_bigram_line(&lines, first))
    };
    match found {
        Some(i) => {
            let to = (i + s_lines.len().min(MAX_QUOTE)).min(lines.len());
            format!(
                "search block not found. Closest match at lines {}-{}:\n{}\nCopy this text exactly into `search` if this is the location you meant.",
                i + 1,
                to,
                lines[i..to].join("\n")
            )
        }
        None => "search block not found - re-read the file and copy the exact text".to_string(),
    }
}

fn best_bigram_line(lines: &[&str], needle: &str) -> Option<usize> {
    let nb = bigrams(needle);
    if nb.is_empty() {
        return None;
    }
    let mut best: Option<(usize, f32)> = None;
    for (i, l) in lines.iter().enumerate() {
        let lb = bigrams(l);
        if lb.is_empty() {
            continue;
        }
        let score = nb.intersection(&lb).count() as f32 / nb.len() as f32;
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((i, score));
        }
    }
    best.filter(|&(_, s)| s >= 0.25).map(|(i, _)| i)
}

fn bigrams(s: &str) -> std::collections::HashSet<(char, char)> {
    let cs: Vec<char> = s.trim().chars().collect();
    cs.windows(2).map(|w| (w[0], w[1])).collect()
}
```

- [ ] **Step 4: 구현 — multi-match**

모호 에러 3곳(1·2·3단계)을 공용 빌더로 교체. 1단계는 바이트 위치를 줄번호로 변환:

```rust
/// 모호 매치 에러에 위치를 나열 (M5 §6.3). line_starts는 0-기준 줄 인덱스
fn ambiguity_message(n: usize, stage: &str, line_starts: &[usize], t_lines: &[&str]) -> String {
    let shown: Vec<String> = line_starts
        .iter()
        .take(5)
        .map(|&i| format!("  line {}: {}", i + 1, t_lines.get(i).copied().unwrap_or("")))
        .collect();
    let more = if line_starts.len() > 5 {
        format!("\n  and {} more", line_starts.len() - 5)
    } else {
        String::new()
    };
    format!(
        "search block matches {n} locations ({stage}):\n{}{more}\nadd surrounding lines to pick one, or set \"replace_all\": true if you intend to change every occurrence",
        shown.join("\n")
    )
}
```

1단계 호출부:

```rust
        n if n >= 2 => {
            let t_lines: Vec<&str> = text.split('\n').collect();
            let starts: Vec<usize> =
                exact_positions.iter().map(|&b| text[..b].matches('\n').count()).collect();
            return Err(ambiguity_message(n, "exact match", &starts, &t_lines));
        }
```

2·3단계 호출부는 이미 줄 인덱스 목록(stage2 / stage3의 `i`들)을 갖고 있으므로 그대로 전달 (stage: `"ignoring trailing whitespace"` / `"with indent shift"`).

- [ ] **Step 5: 기존 테스트 갱신 + 통과 확인**

- `two_exact_matches_is_an_immediate_ambiguity_error`: `"2 locations"` 단언 유지(새 메시지에도 포함) — 무변경 통과 예상
- `not_found_reports_near_miss_line`: `"Line 2"` → `"lines 2-"` 형식으로 단언 갱신

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS

- [ ] **Step 6: 커밋**

```bash
git add src/tools/edit_file.rs
git commit -m "feat(tools): edit_file not-found 최근접 인용 + multi-match 위치 나열 (M5 Batch 2)"
```

---

### Task 11: edit_file — replace_all (스펙 §6.4)

**Files:**
- Modify: `src/tools/edit_file.rs`

**Interfaces:**
- Produces: `Args`에 `#[serde(default)] replace_all: bool`; `apply_edit(text, search, replace, replace_all: bool)`; 성공 메시지 `"Edited {path} (replaced {N} occurrences, matched {mode})\n{첫 매치 컨텍스트}"`; doc() 갱신: `"edit_file(path, search, replace, replace_all?): Replace one occurrence of `search` with `replace` in an existing file. `search` must match exactly one location; include a few surrounding lines to make it unique. Set replace_all=true to replace every occurrence (plain-text match - it also matches inside longer identifiers)."`

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[test]
fn replace_all_replaces_every_exact_occurrence() {
    let (dir, ctx) = setup("total_price(a);\nmid\ntotal_price(b);\n");
    let out = EditFile
        .run(&serde_json::json!({"path": "f.rs", "search": "total_price", "replace": "total", "replace_all": true}), &ctx)
        .unwrap();
    assert!(out.contains("replaced 2 occurrences"), "{out}");
    let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
    assert_eq!(t, "total(a);\nmid\ntotal(b);\n");
}

#[test]
fn replace_all_with_single_match_still_works() {
    let (dir, ctx) = setup("only_one();\n");
    EditFile
        .run(&serde_json::json!({"path": "f.rs", "search": "only_one", "replace": "renamed", "replace_all": true}), &ctx)
        .unwrap();
    assert!(std::fs::read_to_string(dir.path().join("f.rs")).unwrap().contains("renamed"));
}

#[test]
fn replace_all_at_stage_two_handles_trailing_whitespace_locations() {
    // 여러 줄 블록 + 후행 공백 — 1단계(부분문자열)로는 못 잡고 2단계 비중첩 탐욕이 처리
    let (dir, ctx) = setup("x;  \ny;\nmid\nx;  \ny;\n");
    let out = EditFile
        .run(&serde_json::json!({"path": "f.rs", "search": "x;\ny;", "replace": "z;", "replace_all": true}), &ctx)
        .unwrap();
    assert!(out.contains("replaced 2 occurrences"), "{out}");
    let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
    assert_eq!(t, "z;\nmid\nz;\n");
}

#[test]
fn replace_all_applies_per_location_indent_at_stage_three() {
    // 들여쓰기가 다른 두 위치의 여러 줄 블록 — 1·2단계로는 못 잡고 3단계가 각자 indent 적용 (스펙 §6.4)
    let (dir, ctx) = setup("fn a() {\n    if x {\n        one();\n    }\n}\nfn b() {\n            if x {\n                one();\n            }\n}\n");
    let out = EditFile
        .run(&serde_json::json!({"path": "f.rs", "search": "if x {\n    one();\n}", "replace": "if y {\n    two();\n}", "replace_all": true}), &ctx)
        .unwrap();
    assert!(out.contains("replaced 2 occurrences"), "{out}");
    let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
    assert!(t.contains("    if y {\n        two();\n    }"), "4칸 위치에 자기 indent: {t}");
    assert!(t.contains("            if y {\n                two();\n            }"), "12칸 위치에 자기 indent: {t}");
}

#[test]
fn replace_all_on_crlf_file_preserves_crlf() {
    let (dir, ctx) = setup("x\r\ndup\r\ny\r\ndup\r\n");
    EditFile
        .run(&serde_json::json!({"path": "f.rs", "search": "dup", "replace": "D", "replace_all": true}), &ctx)
        .unwrap();
    let t = String::from_utf8(std::fs::read(dir.path().join("f.rs")).unwrap()).unwrap();
    assert_eq!(t, "x\r\nD\r\ny\r\nD\r\n");
}
```

(stage-2·stage-3 테스트의 search는 여러 줄 + 파일 쪽 후행 공백/들여쓰기 차이로 구성되어 1단계 부분문자열 매치로는 잡히지 않는다 — 각 사다리 단계의 replace_all 경로를 실제로 검증)

- [ ] **Step 2: 실패 확인**

Run: `cargo test replace_all`
Expected: FAIL — 미지 인자 무시로 기존 모호 에러 발생

- [ ] **Step 3: 구현**

`Args`에 `#[serde(default)] replace_all: bool` 추가. `apply_edit`에 `replace_all: bool` 파라미터 추가, 각 단계에서:

```rust
    // 1단계
    match exact_positions.len() {
        0 => {}
        1 => { /* 기존 단일 치환 */ }
        n if replace_all => {
            let start_line = text[..exact_positions[0]].matches('\n').count();
            return Ok(EditOutcome {
                new_text: text.replace(search, replace),
                mode: MatchMode::Exact,
                start_line,
                replaced_lines: replace.split('\n').count().max(1),
                occurrences: n,
            });
        }
        n => return Err(ambiguity_message(n, "exact match", &starts, &t_lines)),
    }
```

2단계 (라인 윈도, 비중첩 탐욕 — 겹치는 창은 앞선 것 우선):

```rust
        n if n >= 2 && replace_all => {
            // 비중첩 탐욕: 시작이 직전 창의 끝 이전이면 건너뜀 (M5 §6.4)
            let mut kept: Vec<usize> = Vec::new();
            for &i in &stage2 {
                if kept.last().is_none_or(|&p| i >= p + window) {
                    kept.push(i);
                }
            }
            let mut lines: Vec<String> = t_lines.iter().map(|s| s.to_string()).collect();
            for &i in kept.iter().rev() {
                let repl = replace_lines(replace, "");
                lines.splice(i..i + window, repl);
            }
            return Ok(EditOutcome {
                new_text: lines.join("\n"),
                mode: MatchMode::IgnoreTrailingWs,
                start_line: kept[0],
                replaced_lines: replace_lines(replace, "").len().max(1),
                occurrences: kept.len(),
            });
        }
```

3단계도 동일 패턴이되 위치별 `indent`로 `replace_lines(replace, indent)` 적용 (stage3는 `(i, indent)` 쌍을 이미 가짐). `MatchMode::IndentShift`의 describe에는 첫 매치 indent 사용.

(`Vec::splice` 대신 기존 `splice` 헬퍼를 역순 반복 적용해도 됨 — 뒤에서 앞으로 적용해야 앞 인덱스가 유효)

`run()` 성공 메시지의 `occurrences > 1` 분기는 **Task 9에서 이미 반영됨** — 이 태스크에서는 `apply_edit`가 occurrences를 2+로 채우기만 하면 된다.

`doc()`을 Interfaces의 문자열로 교체. `parse`의 무변경 검사는 replace_all에도 그대로 적용.

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS. `two_exact_matches_is_an_immediate_ambiguity_error`(replace_all 미지정)는 여전히 에러 — 무변경 통과

- [ ] **Step 5: 커밋**

```bash
git add src/tools/edit_file.rs
git commit -m "feat(tools): edit_file replace_all — rename류 다중 발생 치환 (M5 Batch 2)"
```

---

### Task 12: [체크포인트 — 사용자 협조] Batch 2 qwen 측정

Task 8과 동일 절차. 차이만 기록:

- [ ] **Step 1: 사용자에게 측정 준비 요청** (qwen 로드 상태 확인 — Task 8 Step 1과 동일)
- [ ] **Step 2: 측정 실행** (`cargo build --release && ./target/release/loco eval tasks --repeats 3`)
- [ ] **Step 3: 지표 집계** — Task 8 Step 3의 grep 세트 + Batch 2 추가 지표:

```bash
grep -o 'Closest match at lines' $S/run-*.jsonl | wc -l   # 최근접 인용 발동 수
grep -o 'replaced [0-9]* occurrences' $S/run-*.jsonl | wc -l  # replace_all 사용 수
grep -o 'replace_all' $S/run-*.jsonl | wc -l              # 오용 관찰 원자료 (단일 의도 다중 치환은 트랜스크립트 정독으로)
```

- [ ] **Step 4: keep/revert 판정** (직전 측정 = Batch 1 결과 기준, 스펙 §3 규칙)
- [ ] **Step 5: `docs/baselines.md` M5 경과 표에 행 추가 + 커밋** (`docs: M5 Batch 2 qwen 측정 결과`)

---

### Task 13: RepetitionTracker 신규 모듈 (스펙 §7.2 — 단위)

**Files:**
- Create: `src/agent/repetition.rs`
- Modify: `src/agent/mod.rs` (모듈 선언 1줄: `pub mod repetition;`)

**Interfaces:**
- Produces:

```rust
pub struct RepetitionTracker { /* private */ }
pub enum RepetitionVerdict { Ok, InjectCorrection, Stop }
impl RepetitionTracker {
    pub fn new() -> Self;
    /// 디스패치 후 호출. key = "tool|정규화된 args", body = 툴 결과 원문(에러 포함)
    pub fn record(&mut self, key: &str, body: &str) -> RepetitionVerdict;
    /// 동일 에러 첫 문장(첫 마침표까지) 3연속이면 교정문(1회) — record와 별도 호출
    pub fn error_correction(&mut self, tool: &str, body: &str) -> Option<&'static str>;
}
pub const EDIT_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Change strategy: re-read the file, then rewrite it completely with write_file.";
pub const GENERIC_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Step back and try a different approach.";
```

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/repetition.rs` (파일 신규 — 테스트 먼저, 최소 스텁으로 컴파일):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_identical_call_and_result_injects_once_fifth_stops() {
        let mut t = RepetitionTracker::new();
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::InjectCorrection));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok), "교정은 1회만");
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Stop));
    }

    #[test]
    fn period_two_alternation_is_caught() {
        // read↔edit 왕복 (qwen rename-function 실측 패턴) — 연속이 아니어도 윈도가 잡는다
        let mut t = RepetitionTracker::new();
        for _ in 0..2 {
            assert!(matches!(t.record("read_file|a", "same"), RepetitionVerdict::Ok));
            assert!(matches!(t.record("edit_file|x", "Error: not found"), RepetitionVerdict::Ok));
        }
        assert!(matches!(t.record("read_file|a", "same"), RepetitionVerdict::InjectCorrection));
    }

    #[test]
    fn changed_result_resets_the_pattern() {
        // 편집 후 재읽기: 같은 호출이라도 결과가 다르면 무해 (스펙 §7.2)
        let mut t = RepetitionTracker::new();
        for _ in 0..4 {
            t.record("read_file|a", "old content");
        }
        assert!(matches!(t.record("read_file|a", "NEW content"), RepetitionVerdict::Ok));
    }

    #[test]
    fn window_caps_at_eight_entries() {
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        // 8턴 밀어내기 — 오래된 2건이 윈도 밖으로
        for i in 0..8 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::Ok), "윈도 밖 반복은 무효");
    }

    #[test]
    fn same_error_first_sentence_three_times_yields_strategy_correction_once() {
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. Closest match at lines 3-5:\nfoo").is_none());
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. Closest match at lines 8-9:\nbar").is_none(), "첫 문장(첫 마침표까지) 비교 — 뒤의 가변 내용은 무시");
        // 세 번째 — 파일 편집 계열이므로 write_file 권고
        let c = t.error_correction("edit_file", "Error: edit failed: search block not found. etc");
        assert_eq!(c, Some(EDIT_STRATEGY_CORRECTION));
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. etc").is_none(), "1회만");
    }

    #[test]
    fn same_error_via_non_edit_tool_gets_generic_correction() {
        let mut t = RepetitionTracker::new();
        t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        let c = t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        assert_eq!(c, Some(GENERIC_STRATEGY_CORRECTION));
    }

    #[test]
    fn non_errors_and_different_errors_reset_the_streak() {
        let mut t = RepetitionTracker::new();
        t.error_correction("grep", "Error: x");
        t.error_correction("grep", "Error: x");
        assert!(t.error_correction("grep", "ok result").is_none());
        t.error_correction("grep", "Error: x");
        t.error_correction("grep", "Error: x");
        assert!(t.error_correction("grep", "Error: x").is_some(), "리셋 후 다시 3연속");
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test repetition`
Expected: FAIL (스텁)

- [ ] **Step 3: 구현**

```rust
//! 반복 감지 — (호출, 결과 해시) 8턴 윈도 + 동일 에러 연속 스트릭 (M5 스펙 §7.2).
//! 계수는 디스패치 후(결과 확보 시점) — 결과를 예단하는 정지는 두지 않는다.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// 윈도 크기 8: 5회 정지는 사실상 연속 반복(주기 1)에서 도달한다. 엄격한 주기 2
/// 교대는 윈도 내 같은 항목이 최대 4회, 주기 3은 최대 3회라 교정(3회째)+max_turns가
/// 상한 (더 넓히면 "다른 편집 사이 동일한 실패 테스트 결과"를 오정지할 위험)
const WINDOW: usize = 8;

pub const EDIT_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Change strategy: re-read the file, then rewrite it completely with write_file.";
pub const GENERIC_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Step back and try a different approach.";

#[derive(Debug, PartialEq)]
pub enum RepetitionVerdict {
    Ok,
    /// 윈도 내 동일 (호출, 결과) 3회째 — 교정 1회 주입
    InjectCorrection,
    /// 5회째 — RepetitionStop
    Stop,
}

pub struct RepetitionTracker {
    window: VecDeque<(String, u64)>,
    cycle_corrected: bool,
    error_corrected: bool,
    last_error_key: Option<String>,
    error_streak: usize,
}

impl RepetitionTracker {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(WINDOW),
            cycle_corrected: false,
            error_corrected: false,
            last_error_key: None,
            error_streak: 0,
        }
    }

    pub fn record(&mut self, key: &str, body: &str) -> RepetitionVerdict {
        let entry = (key.to_string(), hash_of(body));
        if self.window.len() == WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(entry.clone());
        let count = self.window.iter().filter(|e| **e == entry).count();
        if count >= 5 {
            return RepetitionVerdict::Stop;
        }
        if count == 3 && !self.cycle_corrected {
            self.cycle_corrected = true;
            return RepetitionVerdict::InjectCorrection;
        }
        RepetitionVerdict::Ok
    }

    pub fn error_correction(&mut self, tool: &str, body: &str) -> Option<&'static str> {
        if !body.starts_with("Error:") {
            self.last_error_key = None;
            self.error_streak = 0;
            return None;
        }
        // 동일성 키 = 첫 문장(첫 '.'까지). 개선된 에러들은 첫 줄 안에 가변 내용을
        // 붙이므로(스키마 에코의 키 목록, not-found의 `lines A-B`) 첫 줄 비교는 무력
        let key = body.split('.').next().unwrap_or(body).to_string();
        if self.last_error_key.as_deref() == Some(key.as_str()) {
            self.error_streak += 1;
        } else {
            self.last_error_key = Some(key);
            self.error_streak = 1;
        }
        if self.error_streak >= 3 && !self.error_corrected {
            self.error_corrected = true;
            return Some(if matches!(tool, "edit_file" | "write_file") {
                EDIT_STRATEGY_CORRECTION
            } else {
                GENERIC_STRATEGY_CORRECTION
            });
        }
        None
    }
}

impl Default for RepetitionTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_of(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
```

`src/agent/mod.rs` 상단에 `pub mod repetition;` 추가.

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (clippy가 `Default` 요구 — 위에 포함)

- [ ] **Step 5: 커밋**

```bash
git add src/agent/repetition.rs src/agent/mod.rs
git commit -m "feat(agent): RepetitionTracker — (호출,결과) 윈도·동일 에러 스트릭 (M5 Batch 3)"
```

---

### Task 14: 루프 통합 — 윈도 교체 + finish 편입 (스펙 §7.2·§7.3)

**Files:**
- Modify: `src/agent/mod.rs` (`run` 루프)

**Interfaces:**
- Consumes: `RepetitionTracker`, `RepetitionVerdict`, `SALVAGE_NOTE`, `REPEAT_CORRECTION`
- 동작 계약: ① 계수는 디스패치 후 (호출 키 = salvage 정규화 후 args) ② 거부(Denied) 결과도 계수 ③ summary 없는 finish는 상수 에러 결과로 계수 ④ Stop 시 툴 결과를 세션에 남기고 `RepetitionStop` 반환

- [ ] **Step 1: 실패하는 테스트 작성/갱신**

`src/agent/mod.rs` tests:

```rust
#[tokio::test]
async fn four_reads_then_edit_then_reread_is_not_stopped() {
    // 스펙 §7.2·§8: 결과 해시가 "편집 후 달라진 재읽기"를 정당한 반복으로 구제
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "old").unwrap();
    let read = || ok(&turn("read_file", serde_json::json!({"path": "f.txt"})));
    let write = ok(&turn("write_file", serde_json::json!({"path": "f.txt", "content": "CHANGED"})));
    // finish 2개: Task 15의 검증 넛지가 1차 finish를 반려한다 (Task 14 시점엔 2번째가 남아도 무해)
    let script = Scripted::new(vec![read(), read(), read(), read(), write, read(), ok(&finish("done")), ok(&finish("done"))]);
    let config = Config { max_turns: 25, ..Default::default() };
    let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
    let mut session = new_session(&agent);
    let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
}

#[tokio::test]
async fn summary_less_finish_loop_ends_in_repetition_stop() {
    // gemma chain-edits-0 실측: summary 없는 finish 14연속 — 이제 5회째 정지 (스펙 §7.3)
    let dir = tempfile::tempdir().unwrap();
    let bad = || ok(&turn("finish", serde_json::json!({})));
    let script = Scripted::new(vec![bad(), bad(), bad(), bad(), bad()]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::RepetitionStop), "{outcome:?}");
}

#[tokio::test]
async fn same_error_three_times_injects_strategy_correction() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "content").unwrap();
    // 서로 다른 args의 edit_file이 같은 에러(첫 줄)를 3연속 수신
    let e = |s: &str| ok(&turn("edit_file", serde_json::json!({"path": "f.txt", "search": s, "replace": "y"})));
    let script = Scripted::new(vec![e("no1"), e("no2"), e("no3"), ok(&finish("giving up"))]);
    let config = Config { max_turns: 25, ..Default::default() };
    let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
    let mut session = new_session(&agent);
    let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    assert!(
        session.messages().iter().any(|m| m.content.contains("rewrite it completely with write_file")),
        "전략 교정 주입"
    );
}
```

(별도 guided 헬퍼는 불필요 — `new_session`은 레지스트리와 무관하게 `&Agent<&Scripted>`를 받는다)

기존 테스트 갱신 2건:
- `five_identical_calls_stop_early_with_one_correction`: 무변경 통과 예상 (5회째 디스패치 후 정지 — LLM 응답 5개 소비 동일, 교정 1회 동일). 실패 시 원인 확인 후 단언만 조정
- `different_args_reset_the_repeat_counter`: 이름과 의미가 바뀐다 — 새 설계에서 a,a,b,a,a는 (a,결과) 3회째에 교정이 발화한다. 테스트를 다음으로 교체:

```rust
#[tokio::test]
async fn alternation_no_longer_resets_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let a = || ok(&turn("list_files", serde_json::json!({})));
    let b = || ok(&turn("list_files", serde_json::json!({"depth": 1})));
    let script = Scripted::new(vec![a(), a(), b(), a(), ok(&finish("ok"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)), "3회는 교정, 정지는 아님");
    assert_eq!(
        session.messages().iter().filter(|m| m.content.contains("repeating the same tool call")).count(),
        1,
        "교대에도 불구하고 윈도가 3회째를 잡아 교정 1회"
    );
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test agent::`
Expected: 신규 3개 + 교체 1개 FAIL

- [ ] **Step 3: 구현**

`run()`에서 `last_action_key`/`repeat_count`/`corrected` 세 변수를 제거하고:

```rust
        let mut tracker = repetition::RepetitionTracker::new();
```

finish 분기(summary 없음)를 교체:

```rust
            if turn.action.tool == "finish" {
                match turn.action.args.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => return Ok(AgentOutcome::Finished(s.to_string())),
                    None => {
                        const FINISH_ERR: &str = "Error: finish requires a string `summary` argument, e.g. {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}";
                        // summary 없는 finish도 반복 계수에 편입 (M5 §7.3 — 기존 §3 사각지대 폐지)
                        let key = format!("finish|{}", turn.action.args);
                        let verdict = tracker.record(&key, FINISH_ERR);
                        // InjectCorrection을 버리면 record()가 래치한 실행당 1회 교정 기회가
                        // 소모된다 — 같은 user 메시지에 병합해 반드시 전달 (본선 스펙 §3 연속 user 금지)
                        let body = match verdict {
                            repetition::RepetitionVerdict::InjectCorrection => {
                                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                                format!("{FINISH_ERR}\n{REPEAT_CORRECTION}")
                            }
                            _ => FINISH_ERR.to_string(),
                        };
                        session.push(tool_result_message("finish", &body));
                        if matches!(verdict, repetition::RepetitionVerdict::Stop) {
                            on_event(AgentEvent::Notice("(같은 툴 호출이 반복돼 조기 종료합니다)".to_string()));
                            return Ok(AgentOutcome::RepetitionStop);
                        }
                        turns += 1;
                        continue;
                    }
                }
            }
```

디스패치 전의 기존 반복 감지 블록(키 계산·5회 조기 반환)을 **삭제**. 거부 분기와 디스패치 분기에서 결과 확보 후 계수:

```rust
            // ... gate_preview 거부 분기:
                if let Decision::Deny { reason } = approver.approve(&req) {
                    on_event(AgentEvent::Notice("(거부됨 — 모델에 전달)".to_string()));
                    let body = format!("Denied: {reason}");
                    let (note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
                    session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
                    if stop {
                        return Ok(AgentOutcome::RepetitionStop);
                    }
                    turns += 1;
                    continue;
                }
            // ... 디스패치 분기 (기존 body 계산 뒤):
            let (note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
            session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
            if stop {
                return Ok(AgentOutcome::RepetitionStop);
            }
            turns += 1;
```

공용 헬퍼 (impl Agent 내부, `&self` 불요하면 자유 함수로):

```rust
    /// 디스패치 후 반복 계수 + 노트 조립 (M5 §7.2). 반환: (병합 노트, RepetitionStop 여부)
    fn track_and_note(
        &self,
        tracker: &mut repetition::RepetitionTracker,
        turn: &protocol::ModelTurn,
        body: &str,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> (Option<String>, bool) {
        let mut notes: Vec<&str> = Vec::new();
        if turn.salvaged {
            notes.push(SALVAGE_NOTE);
        }
        let key = format!("{}|{}", turn.action.tool, turn.action.args);
        match tracker.record(&key, body) {
            repetition::RepetitionVerdict::Stop => {
                on_event(AgentEvent::Notice("(같은 툴 호출이 반복돼 조기 종료합니다)".to_string()));
                return (None, true);
            }
            repetition::RepetitionVerdict::InjectCorrection => {
                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                notes.push(REPEAT_CORRECTION);
            }
            repetition::RepetitionVerdict::Ok => {}
        }
        if let Some(strategy) = tracker.error_correction(&turn.action.tool, body) {
            on_event(AgentEvent::Notice("(동일 에러 반복 — 전략 교정 주입)".to_string()));
            notes.push(strategy);
        }
        let joined = notes.join("\n");
        ((!joined.is_empty()).then_some(joined), false)
    }
```

주의: Stop이어도 위 코드처럼 **툴 결과를 세션에 push한 뒤** 반환한다(감사 가능성) — 호출부 순서가 그렇게 배치되어 있다.

- [ ] **Step 4: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS — 특히 `five_identical_calls_stop_early_with_one_correction` 무변경 통과, `salvaged_turn_gets_a_note_with_the_tool_result` 통과 유지

- [ ] **Step 5: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): 반복 감지를 (호출,결과) 윈도로 교체 + finish 편입 (M5 Batch 3)"
```

---

### Task 15: 검증 넛지 (스펙 §7.1)

**Files:**
- Modify: `src/agent/mod.rs`

**Interfaces:**
- Produces: `pub const VERIFY_NUDGE: &str = "You modified files but never ran a verification command. Run the project's tests (e.g. cargo test) with run_command, then finish.";`
- 동작 계약: mutating 툴 디스패치 Ok → 플래그 on; run_command 디스패치 Ok(종료 코드 무관) → 플래그 off; 플래그 on 상태의 summary 있는 finish는 실행당 1회 반려

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[tokio::test]
async fn finish_after_edit_without_verification_is_nudged_once() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![
        ok(&turn("write_file", serde_json::json!({"path": "new.txt", "content": "x"}))),
        ok(&finish("done without verify")),   // 1차 — 반려
        ok(&finish("done anyway")),           // 2차 — 통과
    ]);
    let config = Config { max_turns: 25, ..Default::default() };
    let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
    let mut session = new_session(&agent);
    let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
    match outcome {
        AgentOutcome::Finished(s) => assert_eq!(s, "done anyway", "2차 finish는 무조건 통과"),
        other => panic!("{other:?}"),
    }
    assert!(session.messages().iter().any(|m| m.content.contains("never ran a verification command")));
}

#[tokio::test]
async fn finish_after_edit_and_run_command_is_not_nudged() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![
        ok(&turn("write_file", serde_json::json!({"path": "new.txt", "content": "x"}))),
        ok(&turn("run_command", serde_json::json!({"command": "exit 3"}))), // 실패해도 "검증 실행"
        ok(&finish("verified")),
    ]);
    let config = Config { max_turns: 25, ..Default::default() };
    let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
    let mut session = new_session(&agent);
    let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    assert!(!session.messages().iter().any(|m| m.content.contains("never ran a verification command")));
}

#[tokio::test]
async fn finish_without_any_edit_is_not_nudged() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![ok(&finish("answer only"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    assert!(!session.messages().iter().any(|m| m.content.contains("verification command")));
}
```

(`exit 3` 명령은 unix 셸 전제 — 이 테스트만 `#[cfg(unix)]` 게이트. 크로스플랫폼 무관 항목은 나머지 2개가 커버)

- [ ] **Step 2: 실패 확인**

Run: `cargo test nudged`
Expected: FAIL

- [ ] **Step 3: 구현**

상수 추가:

```rust
/// 무검증 finish 1회 반려 (M5 §7.1). 모델 대상 — 영어
pub const VERIFY_NUDGE: &str = "You modified files but never ran a verification command. Run the project's tests (e.g. cargo test) with run_command, then finish.";
```

`run()`에 상태 2개 추가:

```rust
        let mut mutated_since_verify = false;
        let mut verify_nudged = false;
```

finish 분기의 `Some(s)` 암을 교체:

```rust
                    Some(s) => {
                        if mutated_since_verify && !verify_nudged {
                            verify_nudged = true;
                            on_event(AgentEvent::Notice("(검증 없는 종료 — 확인 요청 주입)".to_string()));
                            session.push(tool_result_message("finish", VERIFY_NUDGE));
                            turns += 1;
                            continue;
                        }
                        return Ok(AgentOutcome::Finished(s.to_string()));
                    }
```

디스패치 결과 처리에서 플래그 갱신 (`let body = match dispatched { ... }` 직전에 성공 여부 캡처):

```rust
            let dispatch_ok = matches!(&dispatched, Ok(Ok(_)));
            // ... body 계산 기존대로 ...
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    mutated_since_verify = false; // 검증 실행으로 인정 — 종료 코드 무관 (M5 §7.1)
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                }
            }
```

- [ ] **Step 4: 넛지로 깨지는 기존 테스트 3건 갱신**

mutating 성공 후 run_command 없이 finish하는 스크립트는 넛지 반려로 LLM 응답이 1개 더
필요해져 `Scripted` 소진 패닉("스크립트에 남은 응답이 없음")이 난다. 갱신 목록:

1. `src/agent/mod.rs` `approved_action_executes` (871행 부근): 스크립트 끝에 두 번째
   finish 응답 추가 (1차 finish는 반려됨). 요청 수를 단언하면 +1
2. `src/eval/mod.rs` `pass_flow_syncs_protected_before_check` (331행 부근): 스크립트에
   두 번째 finish 추가 + `assert_eq!(t.runs[0].turns, 4)`를 5로 갱신 (넛지 반려가 턴 1개 소비)
3. Task 14에서 추가한 `four_reads_then_edit_then_reread_is_not_stopped`: 이미 finish
   응답 2개로 작성돼 있음 — 이제 둘 다 소비된다 (수정 불필요, 통과만 확인)

- [ ] **Step 5: 통과 확인 + 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS — 편집 없는 시나리오(`finish_returns_summary...` 등)는 무변경 통과

- [ ] **Step 6: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): 무검증 finish 1회 반려 넛지 (M5 Batch 3)"
```

---

### Task 16: [체크포인트 — 사용자 협조] Batch 3 qwen 측정

Task 8과 동일 절차 (준비 요청 → `cargo build --release && ./target/release/loco eval tasks --repeats 3` → 지표 집계 → keep/revert). Batch 3 추가 지표:

```bash
grep -o 'never ran a verification command' $S/run-*.jsonl | wc -l  # 넛지 발동 수
grep -o 'The same error keeps occurring' $S/run-*.jsonl | wc -l    # 전략 교정 발동 수
grep -o '"outcome": "[a-z_]*"' $S/report.json | sort | uniq -c     # RepetitionStop/MaxTurns 이동
```

- [ ] Step 1~4: Task 8과 동일
- [ ] Step 5: `docs/baselines.md` M5 경과 표에 행 추가 + 커밋 (`docs: M5 Batch 3 qwen 측정 결과`)

---

### Task 17: [체크포인트 — 사용자 협조] 최종 두 모델 측정 + 문서화

**Files:**
- Modify: `docs/baselines.md` (M5 최종 결과 절), `docs/superpowers/specs/2026-07-02-loco-design.md` (개정 이력), `CLAUDE.md`

- [ ] **Step 1: qwen 최종 측정** — Batch 3 측정(Task 16)을 그대로 최종치로 쓸 수 있으면 재실행 생략 (Batch 3 이후 코드 변경이 없을 때)

- [ ] **Step 2: gemma 측정** — 사용자에게 모델 교체 요청: "qwen 언로드 → google/gemma-4-e4b 단독 로드(ctx 8192)". 확인 후:

```bash
./target/release/loco eval tasks --repeats 3
```

- [ ] **Step 3: 성공 기준 판정 (스펙 §2)**

1. 공통 0% 6종(add-function, chain-edits, implement-from-doc, multiline-string-edit, rename-function, fix-compile-error) 중 ≥2종이 한 모델에서라도 0 탈출?
2. qwen 안정 4종(create-module, edit-crlf-file, find-definition, fix-off-by-one) 각각 ≥2/3?
3. 모델별 전체 통과율 ≥ 기준선(gemma 11.1%, qwen 33.3%)?

- [ ] **Step 4: `docs/baselines.md`에 "M5 최종 결과" 절 작성** — 기준선 문서와 동일 형식(전체 통과율 표, 과제별 표, outcome 분포, 관찰), 성공 기준 3항 판정 명기, 잔여 한계 승계(.cargo 홈·$CARGO_HOME 벡터는 미차단, 트립와이어는 temp_dir/.cargo가 우연히 존재하는 환경에서 하네스를 중단시킴 — 스펙 §4.1)

- [ ] **Step 5: 본선 스펙 개정 이력에 추가**

`docs/superpowers/specs/2026-07-02-loco-design.md` 개정 이력에:

```markdown
- 2026-07-XX: M5 반영 — §3 반복 감지를 (호출,결과 해시) 8턴 윈도로 일반화(교대·finish
  반복 사각지대 해소), 무검증 finish 1회 반려, §4 salvage 파싱·edit_file replace_all·
  에러 피드백 강화, §8 .cargo 암묵 protected·timeout 클램프·config 스냅샷.
  상세: docs/superpowers/specs/2026-07-12-m5-scaffolding-design.md
```

- [ ] **Step 6: CLAUDE.md 갱신** — Architecture 절의 해당 서술 갱신 (반복 감지 서술, edit_file 사다리 서술에 replace_all, eval 절에 .cargo 암묵 protected·config 스냅샷, salvage·넛지 한 줄). 영문 유지.

- [ ] **Step 7: 커밋**

```bash
git add docs/baselines.md docs/superpowers/specs/2026-07-02-loco-design.md CLAUDE.md
git commit -m "docs: M5 최종 측정 결과 및 스펙·CLAUDE.md 반영"
```

- [ ] **Step 8: 완료 보고** — superpowers:finishing-a-development-branch 스킬로 머지/정리 결정 (사용자 승인 필요)

---

## Self-Review 기록

- **스펙 커버리지**: §4.1→Task 2, §4.2→Task 1, §4.3→Task 3, §5.1→Task 4, §5.2→Task 5, §5.3→Task 6, §5.4→Task 7, §6.1·6.5→Task 9, §6.2·6.3→Task 10, §6.4→Task 11, §7.1→Task 15, §7.2→Task 13·14, §7.3→Task 14, §3 측정 프로토콜→Task 8·12·16·17, 본선 스펙 개정→Task 17. 갭 없음
- **타입 일관성**: `EditOutcome`(Task 9 정의, 10·11 사용), `RepetitionTracker::record/error_correction`(Task 13 정의, 14 사용), `with_implicit_protected`/`cargo_tripwire`(Task 2), `scaled_timeout`(Task 1), `EffectiveConfig`(Task 3), `ModelTurn.salvaged`(Task 4 정의, 14 사용) — 시그니처 상호 참조 확인 완료
- **주의 항목**: Task 14는 Task 4가 만든 note 조립부를 대체(선행 의존 명기). grep 폴백(Task 6)은 기존 truncated 경로 보존을 단언으로 강제
- **독립 리뷰 반영(2026-07-12, Ready=No → 수정)**: Critical 1(동일 에러 판정을 첫 문장 기준으로 — Task 10·5의 메시지가 첫 줄 안에 가변 내용 포함), Important — grep 기존 테스트 교체 명시, render_context 후행 빈 줄 제거, occurrences 분기를 Task 9로 당겨 dead_code 회피, 넛지로 깨지는 기존 테스트 3건 갱신 단계 신설, finish 분기의 InjectCorrection 병합 전달, 윈도 8의 주기 2 정지 불가 서술 정정(스펙 동기화). Minor — eval 비게이트 unit_tests 모듈, merge_entry 자유 함수화+미사용 import 삭제, 폴백+절단 마커 보존, 프롬프트 예산 테스트 주의, replace_all 2·3단계 실경로 테스트, 거짓 성공 finish 지표, 트립와이어 사전 점검·한계 기록, preview 갱신 코드 제시, guided 헬퍼 중복 제거
