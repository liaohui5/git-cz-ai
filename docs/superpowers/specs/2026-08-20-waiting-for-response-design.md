# 请求状态提示（waiting-for-response）设计规格

> 日期：2026-08-20 · 状态：已批准

## 需求

`git-cz ai` 子命令在向 LLM API 发送请求后、响应返回前的等待期间，输出提示：

```
Request has been sent, waiting for response
```

## 设计决策（经用户确认）

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 完成提示 | 响应返回后追加 `Response received` | 简单、无终端控制序列，兼容管道/重定向 |
| 输出流 | stderr（`eprintln!`） | 进度类提示惯例；重定向 stdout 时不污染日志文件 |
| 完成提示文案 | `Response received` | 与发送提示对应，中性简洁 |

## 改动

**文件：** `src/main.rs`（仅 `run_ai` 函数，bin 层编排）

**位置：** 步骤 3（发送请求到 LLM API）

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

## 行为规格

| 场景 | 输出（stderr） |
|------|----------------|
| 请求发出 | `Request has been sent, waiting for response` |
| 响应成功返回 | 追加 `Response received` |
| HTTP 错误（非 2xx） | 仅发送提示 + 现有 `llm api error: HTTP <code>: <text>`，退出码 1 |
| 网络/传输错误 | 仅发送提示 + 现有 `llm api request failed: <err>`，退出码 1 |
| 响应解析失败 | 发送提示 + `Response received` + 现有解析错误，退出码 1（解析失败发生在响应收到之后） |

## 边界与理由

- **发送提示在 `send_json` 之前打印**：ureq 是同步阻塞调用，调用后直到响应返回才继续执行；在调用前打印即请求发出瞬间展示。
- **失败路径不打印完成提示**：HTTP/传输错误分支无响应可收，现有错误输出已足够明确。
- **promkit 兼容**：两条提示均在候选选择器（`QuerySelector`）启动前打印完毕，不干扰终端重绘。

## 验证

- **手动验证（mock LLM API）**：
  1. 正常响应：依次看到两行提示 → 候选列表 → 提交
  2. HTTP 错误（mock 返回 500）：发送提示 + `llm api error`，退出码非零
  3. 网络错误（端点不可达）：发送提示 + `llm api request failed`，退出码非零
- **自动化测试**：不新增——`run_ai` 是 bin 层编排（与现有模式一致，无自动化测试）；提示输出为终端交互行为，手动验证覆盖。

## 非目标（YAGNI）

- 不做 `\r` 覆盖/动态状态行（用户已确认不需要）
- 不改 lib 层、不改依赖、不改 CLI 参数
