# Agent Usage Module — Smoke Test 3/15

## Setup

1. Create credentials file at `~/.config/ferritebar/agents.json`:

```json
{
  "claude": {
    "access_token": "",
    "refresh_token": ""
  },
  "codex": {
    "access_token": "",
    "refresh_token": "",
    "account_id": ""
  }
}
```

Token sources:
- **Claude**: OAuth tokens from claude.ai (same ones CodexBar-android uses)
- **Codex**: ChatGPT tokens + account ID (same ones CodexBar-android uses)

2. Add module to `config.toml`:

```toml
[[modules.right]]
type = "agent_usage"
interval = 120
```

## Tests

### T1 — Both services configured
- [ ] Add both claude and codex credentials to `agents.json`
- [ ] Launch ferritebar
- [ ] Hover over the agent usage icon
- [ ] Tooltip shows two lines with format:
  ```
  Codex 5hr XX% | Week XX% Resets M/DD
  Claude 5hr XX% | Week XX% Resets M/DD
  ```
- [ ] Percentages represent **remaining** usage (high = good)

### T2 — Only one service configured
- [ ] Remove the `codex` key from `agents.json` (or set to `null`)
- [ ] Restart ferritebar
- [ ] Tooltip shows `Codex: not configured` and valid Claude line

### T3 — Expired token refresh
- [ ] Set `access_token` to `"expired"` for one service, keep valid `refresh_token`
- [ ] Restart ferritebar
- [ ] Tooltip should still show valid data (token auto-refreshed)
- [ ] Check `agents.json` — `access_token` should be updated with new value

### T4 — Bad credentials
- [ ] Set both `access_token` and `refresh_token` to `"garbage"`
- [ ] Restart ferritebar
- [ ] Tooltip shows error message, no crash

### T5 — Missing file
- [ ] Rename/delete `agents.json`
- [ ] Restart ferritebar
- [ ] Tooltip shows file read error, no crash

### T6 — Polling
- [ ] Set `interval = 60` in config
- [ ] Wait >60s
- [ ] Tooltip data updates without restart
