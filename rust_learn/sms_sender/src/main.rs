use reqwest::Client;  
use reqwest::Error;  
use reqwest::header::{CONTENT_TYPE, HeaderValue};  
use serde::{Deserialize, Serialize};  
use serde_json::Value;  
use std::env;  
use std::collections::HashMap;  
use tokio; // 引入tokio运行时  
  
// 请求结构体，根据API文档调整字段  
#[derive(Serialize)]  
struct SmsRequest {  
    ip: String,  
    mess: String,  
    platformno: String,  
    port: String,  
    userlist: Vec<String>, // 手机号列表  
}  
  
// 假设API响应可能包含这样的结构体（根据你的API文档调整）  
#[derive(Deserialize)]  
struct ApiResponse {  
    status: String,  
    message: String,  
}  
  
#[tokio::main] // 标记main函数为异步入口点
async fn main() {  
    // 从命令行参数获取参数，第一个参数是程序名，后面依次是mess, 一个或多个手机号  
    let args: Vec<String> = env::args().collect();  
    if args.len() < 3 {  
        eprintln!("Usage: {} <ip> <mess> <platformno> <port> <phone_number1> [<phone_number2> ...]", args[0]);  
        std::process::exit(1);  
    }  
  
    // let ip = &args[1];  
    let mess = &args[1];  
    // let platformno = &args[3];  
    // let port = &args[4];  
    let phone_numbers: Vec<String> = args[2..].to_vec(); // 收集所有剩余的参数作为手机号列表  
  
    // 创建SmsRequest实例  
    let sms_request = SmsRequest {  
        ip: "10.11.26.210".to_string(),  
        mess: mess.to_string(),  
        platformno: "zabbix".to_string(),  
        port: "0".to_string(),  
        userlist: phone_numbers,  
    };  
  
    // 你的短信API的URL  
    let api_url = "http://10.11.26.115:8080/api/aiops/message/template/pushSMS";  
  
    // 创建reqwest客户端并发送POST请求  
    let client = Client::new();  
    let response = client  
        .post(api_url)  
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))  
        .json(&sms_request) // 将SmsRequest序列化为JSON并作为请求体发送  
        .send()
        .await;  
  
    // 处理响应  
    match response {  
        Ok(res) => {  
            if res.status().is_success() {  
                let json: ApiResponse = res.json().expect("Failed to parse response");  
                println!("API Response: {:?}", json);  
            } else {  
                eprintln!("API Error: {}", res.status());  
            }  
        }  
        Err(Error::RequestError(e)) => {  
            eprintln!("Request error: {}", e);  
        }  
        Err(e) => {  
            eprintln!("Error: {}", e);  
        }  
    }  
}
