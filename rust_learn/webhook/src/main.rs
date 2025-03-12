use clap::Parser;
use reqwest::Client;
use serde::Serialize;
use std::error::Error;

#[derive(Serialize)]
struct TextContent {
    text: String,
}

#[derive(Serialize)]
struct WebhookMessage {
    msg_type: String,
    content: TextContent,
}

#[derive(Parser)]
#[clap(about, version, author)]
struct Args {
    /// Webhook URL
    webhook_url: String,

    /// Message to send
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // 创建消息内容
    let message = WebhookMessage {
        msg_type: "text".to_string(),
        content: TextContent {
            text: args.message,
        },
    };

    // 创建 HTTP 客户端并使用固定的代理地址
    let client = Client::builder()
        .proxy(reqwest::Proxy::all("http://172.18.1.214:8087")?)
        .build()?;

    // 发送 POST 请求
    let response = client
        .post(&args.webhook_url)
        .json(&message)
        .send()
        .await?;

    // 检查响应状态
    if response.status().is_success() {
        println!("消息发送成功！");
    } else {
        println!("消息发送失败，状态码: {}", response.status());
        let error_body = response.text().await?;
        eprintln!("错误响应: {}", error_body);
    }

    Ok(())
}
