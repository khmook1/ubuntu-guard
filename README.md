# server-guard

**EC2 · Ubuntu 서버를 위한 경량 보안 모니터링 에이전트**

서버에 설치되면 백그라운드에서 SSH 인증 등 보안 이벤트를 실시간으로 감시하고, 설정한 중앙 서버로 상태를 전송합니다. APT(`.deb`)로 설치하고 systemd로 자동 실행됩니다.

---

## 이런 팀에 맞습니다

- Ubuntu EC2를 운영하며 **SSH 침해 시도·로그인 성공**을 빠르게 받고 싶을 때
- 에이전트가 **가벼워야** 하고, 서버 장애 시에도 모니터링이 끊기지 않아야 할 때
- 자체 백엔드/슬랙·알림 파이프라인으로 **웹훅 JSON**을 받아 처리하고 싶을 때

## 주요 기능

| 기능          | 설명                                                              |
| ------------- | ----------------------------------------------------------------- |
| SSH 감시      | journald 기반 로그인 성공·실패·root 로그인 등 실시간 감지         |
| 웹훅 전송     | `webhook_api_url`로 이벤트·heartbeat POST (미설정 시 로컬 감시만) |
| Heartbeat     | 주기적으로 에이전트 생존·버전·호스트 정보 전송                    |
| 자동 운영     | 부팅 시 기동, 비정상 종료 시 재시작, journald 로그                |
| 안전한 기본값 | 최소 권한 systemd, 설정 오류 시 무한 재시작 방지                  |
| 다국어 로그   | 운영 로그 `en` / `ko` 전환                                        |

에이전트는 **HTTP 클라이언트**로만 동작합니다. 서버에 포트를 열지 않습니다.

---

## 빠른 설치 (Ubuntu / EC2)

### 1. 패키지 설치

```bash
sudo dpkg -i server-guard_0.1.0_amd64.deb
# ARM 인스턴스: server-guard_0.1.0_arm64.deb
```

설치 경로:

| 경로                                       | 내용            |
| ------------------------------------------ | --------------- |
| `/usr/bin/server-guard`                    | 실행 파일       |
| `/etc/server-guard/config.toml`            | 에이전트 설정   |
| `/etc/server-guard/settings.toml`          | 언어·웹훅 설정  |
| `/etc/server-guard/environment`            | 비밀값(토큰 등) |
| `/lib/systemd/system/server-guard.service` | systemd 유닛    |

### 2. 웹훅 설정

패키지 설치 시 `/etc/server-guard/settings.toml`이 생성됩니다.

```toml
[locale]
language = "ko"   # en | ko

# 중앙 서버 웹훅 (비우면 원격 전송 없이 로컬 감시만 수행)
webhook_api_url = "https://your-server.example.com/hooks/server-guard"
```

또는 환경 파일:

```bash
sudoedit /etc/server-guard/environment
# SERVER_GUARD_WEBHOOK_API_URL=https://your-server.example.com/hooks/server-guard
```

### 3. 서비스 시작

```bash
sudo systemctl start server-guard
sudo systemctl status server-guard
sudo journalctl -u server-guard -f
```

`webhook_api_url`(또는 동등한 환경변수)이 유효하면 설치 직후 자동 시작될 수 있습니다.  
미설정 시 서비스는 **enable만** 되고, 설정 후 수동으로 시작하면 됩니다.

---

## 동작 방식

```text
┌─────────────────────────────┐
│  Ubuntu EC2                 │
│  ┌───────────────────────┐  │         HTTPS POST
│  │ server-guard (systemd)│──┼──────────────────────►  중앙 서버 / 웹훅
│  │  · SSH journal 감시    │  │         이벤트·heartbeat
│  │  · heartbeat          │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

1. systemd가 에이전트를 상시 실행합니다.
2. journald에서 SSH 인증 로그를 추적합니다.
3. 이벤트가 발생하면 JSON으로 웹훅(및 선택적 API)에 전송합니다.
4. 웹훅·API가 일시 장애여도 **감시는 계속**됩니다. 에이전트는 종료하지 않습니다.

---

## 전송되는 이벤트

| 이벤트                            | 의미              |
| --------------------------------- | ----------------- |
| `SSH_LOGIN_SUCCESS`               | SSH 로그인 성공   |
| `SSH_LOGIN_FAILURE`               | SSH 로그인 실패   |
| `SSH_ROOT_LOGIN`                  | root 로그인 성공  |
| `SSH_AUTH_FAILURE`                | 기타 인증 실패    |
| `AGENT_STARTED` / `AGENT_STOPPED` | 에이전트 수명주기 |
| `AGENT_HEARTBEAT`                 | 주기적 생존 신호  |

### 페이로드 예시

보안 이벤트는 JSON 본문으로 POST됩니다. Heartbeat는 아래 형태입니다.

```json
{
  "type": "AGENT_HEARTBEAT",
  "payload": {
    "agent_id": "...",
    "version": "0.1.0",
    "status": "running",
    "hostname": "ip-10-0-0-12",
    "timestamp": "2026-11-18T06:00:00Z"
  }
}
```

수신 측에서는 HTTP `2xx`를 반환하면 됩니다.

---

## 설정 가이드

### settings.toml (권장 — 웹훅·언어)

| 키                | 설명                                   |
| ----------------- | -------------------------------------- |
| `locale.language` | 로그 언어 (`en` \| `ko`)               |
| `webhook_api_url` | 중앙 서버 웹훅 URL. 비우면 웹훅 비활성 |

### config.toml (에이전트 동작)

| 키                           | 설명                           |
| ---------------------------- | ------------------------------ |
| `agent.id`                   | 에이전트 ID (비우면 자동 생성) |
| `agent.log_level`            | 로그 레벨 (`info` 등)          |
| `api.base_url`               | (선택) REST API base URL       |
| `api.timeout_seconds`        | HTTP 타임아웃                  |
| `monitor.ssh_enabled`        | SSH 모니터 on/off              |
| `heartbeat.enabled`          | heartbeat on/off               |
| `heartbeat.interval_seconds` | heartbeat 주기(초)             |

예제 파일: [`config/config.example.toml`](config/config.example.toml), [`config/settings.example.toml`](config/settings.example.toml)

### 환경변수

| 변수                           | 용도                         |
| ------------------------------ | ---------------------------- |
| `SERVER_GUARD_WEBHOOK_API_URL` | 웹훅 URL (settings보다 우선) |
| `SERVER_GUARD_LANGUAGE`        | 로그 언어                    |
| `SERVER_GUARD_SETTINGS`        | settings.toml 경로           |
| `SERVER_GUARD_CONFIG`          | config.toml 경로             |
| `SERVER_GUARD_AGENT_ID`        | 에이전트 ID 고정             |
| `SERVER_GUARD_LOG_LEVEL`       | 로그 레벨                    |
| `SERVER_GUARD_API_TOKEN`       | (선택) API Bearer 토큰       |
| `SERVER_GUARD_API_BASE_URL`    | (선택) API base URL          |

토큰·비밀값은 예제 파일에 넣지 마세요. `/etc/server-guard/environment` (권한 `0640`)를 사용하세요.

---

## 운영

```bash
sudo systemctl start server-guard
sudo systemctl stop server-guard
sudo systemctl restart server-guard
sudo systemctl status server-guard
sudo journalctl -u server-guard -f
```

| 항목          | 동작                                                         |
| ------------- | ------------------------------------------------------------ |
| 부팅 시       | `multi-user.target`에 연결 · 자동 기동                       |
| 프로세스 장애 | `Restart=on-failure` (설정 오류 exit `78`은 재시작하지 않음) |
| 로그          | journald (`journalctl -u server-guard`)                      |
| 권한          | `server-guard` 전용 사용자, journal 읽기용 최소 그룹         |

---

## 패키지 받기 · 빌드

GitHub Releases에서 아키텍처별 `.deb`와 SHA256을 제공합니다.

```bash
# 태그 푸시 시 CI가 amd64 / arm64 패키지를 생성합니다
git tag v0.1.0
git push origin v0.1.0
```

직접 빌드:

```bash
./scripts/build-deb.sh amd64
./scripts/build-deb.sh arm64
```

macOS에서 Linux용 패키지를 만들 때는 `cargo-zigbuild` 사용을 권장합니다.

```bash
cargo install cargo-zigbuild
TARGET=x86_64-unknown-linux-gnu ./scripts/build-deb.sh amd64
```

산출물: `dist/server-guard_<version>_<arch>.deb`

---

## 제거

```bash
sudo systemctl stop server-guard
sudo dpkg -r server-guard          # 설정 파일 유지
sudo dpkg --purge server-guard     # 설정·상태까지 삭제
```

업그레이드 시 `config.toml` · `settings.toml`은 conffile로 보존됩니다.

---

## 개발자 노트

- Language: Rust (Edition 2024) · Async: Tokio · HTTP: Reqwest (rustls)
- 단위 테스트는 SSH 실접속 없이 로그 샘플로 검증합니다.

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo run -- config/config.example.toml
```

macOS에서는 SSH journal 감시가 동작하지 않고 종료 시그널을 대기합니다. 실제 감시는 Ubuntu에서 확인하세요.

---

## 라이선스

MIT
