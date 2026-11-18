use super::Msg;

pub fn translate(key: Msg) -> &'static str {
    match key {
        Msg::API_ERROR => {
            "API가 완전히 설정되지 않았습니다(base_url/token). \
             이벤트는 로컬에서만 처리되며 전송되지 않습니다. \
             SERVER_GUARD_API_TOKEN과 예제가 아닌 api.base_url을 설정하세요."
        }
        Msg::AGENT_STARTING => "server-guard 에이전트를 시작합니다",
        Msg::AGENT_STOPPED => "server-guard 에이전트가 중지되었습니다",
        Msg::CONFIG_LOADED => "설정을 불러왔습니다",
        Msg::RUNTIME_START_FAILED => "비동기 런타임 시작에 실패했습니다",
        Msg::AGENT_TERMINATED => "에이전트가 오류로 종료되었습니다",
        Msg::SSH_MONITOR_DISABLED => "설정에 의해 SSH 모니터가 비활성화되었습니다",
        Msg::SHUTDOWN_SIGNAL => "종료 시그널을 수신했습니다",
        Msg::TASK_ENDED_UNEXPECTEDLY => "백그라운드 작업이 예기치 않게 종료되었습니다",
        Msg::SSH_MONITOR_STARTED => "SSH 모니터를 시작했습니다",
        Msg::SSH_MONITOR_STOPPED => "SSH 모니터를 중지했습니다",
        Msg::SSH_JOURNAL_RECONNECT => "SSH journal 소스가 종료되었습니다. {delay} 후 재연결합니다",
        Msg::SSH_PERMISSION_DENIED => "{msg}",
        Msg::SSH_MONITOR_ERROR => "SSH 모니터 오류: {error}. {delay} 후 재연결합니다",
        Msg::SSH_LINUX_ONLY => {
            "SSH journal 모니터링은 Linux 전용입니다. 이 플랫폼에서는 종료를 대기합니다"
        }
        Msg::SSH_EVENT_ENQUEUE_FAILED => "SSH 이벤트 큐 적재에 실패했습니다: {error}",
        Msg::SSH_LOG_IGNORED => "인증과 무관한 SSH 로그 라인을 무시했습니다",
        Msg::HEARTBEAT_STARTED => "heartbeat 서비스를 시작했습니다",
        Msg::HEARTBEAT_STOPPED => "heartbeat 서비스를 중지했습니다",
        Msg::HEARTBEAT_SENT => "heartbeat를 전송했습니다",
        Msg::HEARTBEAT_FAILED => "heartbeat 전송에 실패했습니다. 다시 시도합니다",
        Msg::API_SKIP_EVENT => "API가 설정되지 않아 이벤트 전송을 건너뜁니다",
        Msg::API_EVENT_PUBLISHED => "이벤트를 전송했습니다",
        Msg::API_SKIP_HEARTBEAT => "API가 설정되지 않아 heartbeat를 건너뜁니다",
        Msg::API_FLUSH_FAILED => "종료 중 이벤트 플러시에 실패했습니다",
        Msg::API_PUBLISH_FAILED => "이벤트 전송에 실패했습니다",
        Msg::WEBHOOK_ENABLED => "웹훅 전송이 활성화되었습니다",
        Msg::WEBHOOK_DISABLED => "webhook_api_url이 비어 있어 웹훅 전송을 사용하지 않습니다",
        Msg::WEBHOOK_SENT => "웹훅을 전송했습니다",
        Msg::WEBHOOK_FAILED => "웹훅 전송에 실패했습니다. 모니터링은 계속합니다",
        Msg::WEBHOOK_SKIP => "웹훅이 설정되지 않아 전송을 건너뜁니다",
        Msg::DELIVERY_NOT_CONFIGURED => {
            "전송 대상이 없습니다(webhook_api_url / api). \
             모니터링은 로컬에서만 계속됩니다"
        }
        Msg::RECEIVED_SIGTERM => "SIGTERM을 수신했습니다",
        Msg::RECEIVED_SIGINT => "SIGINT를 수신했습니다",
        Msg::RECEIVED_CTRL_C => "Ctrl-C를 수신했습니다",
    }
}
