# 请求状态提示（waiting-for-response）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** `git-cz ai` 子命令在 LLM 请求发出后、响应返回前输出 `Request has been sent, waiting for response`（stderr），响应成功后追加 `Response received`。

**架构：** 仅修改 `src/main.rs` 的 `run_ai`（bin 层编排）：在 `send_json` 调用前打印发送提示（ureq 同步阻塞，调用后直至响应返回才继续），在 `match` 的 `Ok` 分支开头打印完成提示；HTTP/传输错误分支不打印完成提示（现有错误输出已足够）。

**技术栈：** Rust + ureq 2（无新依赖、无 lib 层改动）。

**设计规格：** `docs/superpowers/specs/2026-08-20-waiting-for-response-design.md`

---

## 文件结构

| 文件 | 动作 | 职责 |
|------|------|------|
| `src/main.rs` | 修改（`run_ai` 函数步骤 3，约 57-76 行） | 添加两条 stderr 提示输出 |

> 说明：`run_ai` 是 bin 层终端编排（项目现有模式：无自动化测试，见 AGENTS.md §5.5「run_ai（bin 层编排，无自动化测试）」），因此本计划用**编译检查 + 全部既有测试回归 + 手动验证（mock LLM）** 作为验证手段，不新增自动化测试。

---

### 任务 1：实现请求状态提示

**文件：**
- 修改：`src/main.rs`（`run_ai` 步骤 3，第 56-76 行）

- [ ] **步骤 1：修改 `run_ai` 添加提示输出**

在 `src/main.rs` 中，将第 56-76 行的步骤 3 代码块替换为：

```rust
    // 3. 发送请求到 LLM API
    eprintln!("Request has been sent, waiting for response");
    let response = ureq::post(&args.api_endpoint)
        .set("Authorization", &format!("Bearer {}", args.api_token))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": args.api_model,
            "messages": [{ "role": "user", "content": prompt }],
        }));

    let body = match response {
        Ok(resp) => {
            eprintln!("Response received");
            resp.into_string()?
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            eprintln!("llm api error: HTTP {}: {}", code, text);
            std::process::exit(1);
        }
        Err(ureq::Error::Transport(e)) => {
            eprintln!("llm api request failed: {}", e);
            std::process::exit(1);
        }
    };
```

要点：
- 发送提示在 `send_json` **之前**打印（ureq 同步阻塞，调用后直至响应返回才继续执行）
- 完成提示在 `Ok` 分支**开头**打印（收到 HTTP 响应即成立）
- 失败分支（`Status` / `Transport`）**不打印**完成提示，保持现有错误输出与退出码

- [ ] **步骤 2：编译检查**

运行：`cargo build`
预期：编译通过，无警告（bin `git-cz`）

- [ ] **步骤 3：运行全部测试确认无回归**

运行：`cargo test`
预期：23 passed；0 failed（既有测试不受影响——lib 层未改动）

- [ ] **步骤 4：Commit**

```bash
git add src/main.rs
git commit -m "feat: show waiting-for-response prompt in ai subcommand"
```

---

### 任务 2：手动验证（mock LLM API）

**文件：**
- 创建（临时，不入库）：`/tmp/mock_llm_status.py`（可返回 200 或 500 的 mock 服务器）

- [ ] **步骤 1：创建可配置状态的 mock 服务器**

创建 `/tmp/mock_llm_status.py`（不入库）：

```python
#!/usr/bin/env python3
# 用法: python3 /tmp/mock_llm_status.py [ok|error]   默认 ok 返回 200
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

mode = sys.argv[1] if len(sys.argv) > 1 else "ok"

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        if mode == "error":
            body = b'{"error":"boom"}'
            self.send_response(500)
        else:
            body = json.dumps({
                "choices": [{"message": {"content": json.dumps([
                    "feat: add login endpoint",
                    "fix(auth): validate token",
                    "docs: update readme",
                ])}}]
            }).encode()
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

HTTPServer(("127.0.0.1", 8124), Handler).serve_forever()
```

- [ ] **步骤 2：场景 A —— 正常响应显示两行提示**

```bash
python3 /tmp/mock_llm_status.py ok &
# 在测试仓库中（确保 user.name/user.email 已配置，且有 staged changes）：
# echo x > a.txt && git add a.txt
target/debug/git-cz ai \
  --api-endpoint=http://127.0.0.1:8124/v1/chat/completions \
  --api-token=sk-test --api-model=gpt-test
```

预期（stderr）：
```
Request has been sent, waiting for response
Response received
```
随后出现候选列表；Enter 提交成功，`git log -1` 验证提交消息。

- [ ] **步骤 3：场景 B —— HTTP 错误不显示完成提示**

```bash
kill %1   # 停掉 ok mock
python3 /tmp/mock_llm_status.py error &
target/debug/git-cz ai \
  --api-endpoint=http://127.0.0.1:8124/v1/chat/completions \
  --api-token=sk-test --api-model=gpt-test
```

预期（stderr）：`Request has been sent, waiting for response` + `llm api error: HTTP 500: ...`，**无** `Response received`，退出码非零。

- [ ] **步骤 4：场景 C —— 网络错误不显示完成提示**

```bash
kill %1   # 停掉 error mock，不启动任何服务器
target/debug/git-cz ai \
  --api-endpoint=http://127.0.0.1:8124/v1/chat/completions \
  --api-token=sk-test --api-model=gpt-test
```

预期（stderr）：`Request has been sent, waiting for response` + `llm api request failed: ...`，**无** `Response received`，退出码非零。

- [ ] **步骤 5：清理**

```bash
pkill -f mock_llm_status.py; rm -f /tmp/mock_llm_status.py
```

> 本任务只验证，无代码变更，无需 commit。

---

## 自检记录

- **规格覆盖度**：设计规格的行为表（发送提示 / 完成提示 / HTTP 错误 / 传输错误）→ 任务 1 实现 + 任务 2 场景 A/B/C 逐一验证。✅
- **占位符扫描**：所有步骤含具体代码或具体命令，无「TODO」「待定」。✅
- **类型一致性**：无新类型/函数签名；仅复用现有 `run_ai` 内 `ureq`、`serde_json` 调用。✅
- **YAGNI**：不做 `\r` 覆盖、不动 lib 层、不加依赖、不加自动化测试（bin 编排现有模式）。✅
