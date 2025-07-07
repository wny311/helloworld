use std::env;
use std::time::Duration;

use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

/// 命令行参数结构
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// 接收的报警信息参数（以 Problem 或 Resolved 开头）
    #[arg(required = true, index = 1)]
    message: String,
}

#[derive(Debug, Serialize)]
struct ModelRequest {
    inputs: Inputs,
    response_mode: String,
    user: String,
    query: String,
}

#[derive(Debug, Serialize)]
struct Inputs {
    query: String,
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    answer: String,
}

/// 飞书消息结构
#[derive(Debug, Serialize)]
struct FeishuMessage {
    msg_type: String,
    content: Content,
}

#[derive(Debug, Serialize)]
struct Content {
    text: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let args = Args::parse();
    let client = Client::new();

    // 根据消息前缀处理不同场景
    if args.message.starts_with("Problem") {
        // 带超时调用大模型
        match timeout(Duration::from_secs(60), query_model(&client, &args.message)).await {
            Ok(Ok(model_answer)) => {
                let combined = format!(
                    "{}\n\nAI 分析结果：\n{}\n\n<at user_id=\"all\">所有人</at>",
                    args.message,
                    model_answer
                );
                info!("Sending combined message to Feishu: {}", combined);
                if let Err(e) = send_feishu(&client, &combined).await {
                    error!("Feishu 发送失败: {:?}", e);
                }
            }
            Ok(Err(e)) => {
                error!("模型调用失败: {:?}", e);
                let err_msg = format!("{} \n\n[模型调用失败] {:?}", args.message, e);
                let _ = send_feishu(&client, &err_msg).await;
            }
            Err(_) => {
                error!("模型响应超时");
                let timeout_msg = format!("{} \n\n[错误] 模型响应超时", args.message);
                let _ = send_feishu(&client, &timeout_msg).await;
            }
        }
    } else if args.message.starts_with("Resolved") {
        info!("Resolved 消息，直接转发：{}", args.message);
        send_feishu(&client, &args.message).await?;
    } else {
        error!("未识别的消息类型：{}", args.message);
        eprintln!("格式要求应以 Problem 或 Resolved 开头");
        return Err("不支持的报警格式".into());
    }

    Ok(())
}

/// 调用大模型，并打印请求/响应 JSON
async fn query_model(
    client: &Client,
    message: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // 从环境变量读取 API Key
    let api_key = env::var("MODEL_API_KEY")
        .expect("请在环境变量中设置 MODEL_API_KEY");
    let url = env::var("MODEL_API_URL")
        .unwrap_or_else(|_| "http://172.20.60.200/v1/completion-messages".into());

    let request = ModelRequest {
        inputs: Inputs {
            query: message.to_string(),
        },
        response_mode: "blocking".into(),
        user: "zabbix".into(),
        query: "请帮助解决问题".into(),
    };

    // 打印请求 JSON
    let req_json = serde_json::to_string_pretty(&request)?;
    info!("请求大模型 (POST {}):\n{}", url, req_json);

    // 先拿到文本，再打印，并反序列化
    let resp_text = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type","application/json")
        .json(&request)
        .send()
        .await?
        .text()
        .await?;

    info!("大模型原始响应:\n{}", resp_text);

    // 反序列化
    let model_resp: ModelResponse = serde_json::from_str(&resp_text)?;
    Ok(model_resp.answer)
}

/// 发送飞书消息
async fn send_feishu(
    client: &Client,
    message: &str,
) -> Result<(), reqwest::Error> {
    let webhook_url = env::var("FEISHU_WEBHOOK_URL")
        .expect("请在环境变量中设置 FEISHU_WEBHOOK_URL");

    let msg = FeishuMessage {
        msg_type: "text".into(),
        content: Content {
            text: message.to_string(),
        },
    };

    info!("Posting to Feishu webhook: {}", webhook_url);
    client.post(&webhook_url).json(&msg).send().await?;
    Ok(())
}

