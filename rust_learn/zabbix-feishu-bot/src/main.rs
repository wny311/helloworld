use std::time::Duration;
use serde::{Deserialize, Serialize};
use clap::Parser;
use reqwest::Client;

// 命令行参数结构
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 接收的报警信息参数（以Problem或Resolved开头）
    #[arg(required = true, index = 1)]
    message: String,
}

// 大模型请求结构
#[derive(Debug, Serialize)]
struct ModelRequest {
    inputs: Inputs,
    response_mode: String,
    user: String,
    query: String,
}

#[derive(Debug, Serialize)]
struct Inputs {
    problem: String,
}

// 大模型响应结构
#[derive(Debug, Deserialize)]
struct ModelResponse {
    answer: String,
    // 其他字段根据需要可以添加
}

// 飞书消息结构
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
    let args = Args::parse();
    let client = Client::new();

    // 根据消息前缀处理不同场景
    match args.message.as_str() {
        msg if msg.starts_with("Problem") => {
            // 调用大模型（带1分钟超时）
            let model_response = match tokio::time::timeout(
                Duration::from_secs(60),
                query_model(&client, &args.message)
            ).await {
                Ok(Ok(res)) => res.answer,
                Ok(Err(e)) => format!("[模型调用失败] {}", e),
                Err(_) => "[错误] 模型响应超时".to_string(),
            };

            // 拼接消息
            let combined_msg = format!(
                "{}\n\nAI分析结果：\n{}\n\n<at user_id=\"all\">所有人</at> ",
                args.message, model_response
            );

            send_feishu(&client, &combined_msg).await?;
        }
        msg if msg.starts_with("Resolved") => {
            // 直接发送原始消息到飞书
            send_feishu(&client, &args.message).await?;
        }
        _ => {
            eprintln!("错误：未识别的消息类型\n格式要求应以 Problem 或 Resolved 开头");
            return Err("不支持的报警格式".into());
        }
    }

    Ok(())
}

async fn query_model(client: &Client, message: &str) -> Result<ModelResponse, reqwest::Error> {
    let api_key = "app-t6gQF5UPPNZqIL71JRrfYPze"; // 建议改为从环境变量读取
    let url = "http://172.20.60.200/v1/completion-messages";
    
    let request = ModelRequest {
        inputs: Inputs {
            problem: message.to_string(),
        },
        response_mode: "blocking".to_string(),
        user: "zabbix".to_string(),
        query: "请帮我解决问题".to_string(),
    };

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request)
        .send()
        .await?
        .json::<ModelResponse>()
        .await?;

    Ok(response)
}

async fn send_feishu(client: &Client, message: &str) -> Result<(), reqwest::Error> {
    let webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/c0fc37f8-dc0a-40a2-b586-8e3e376ff507";
    
    let msg = FeishuMessage {
        msg_type: "text".to_string(),
        content: Content {
            text: message.to_string(),
        },
    };

    client
        .post(webhook_url)
        .json(&msg)
        .send()
        .await?;

    Ok(())
}
