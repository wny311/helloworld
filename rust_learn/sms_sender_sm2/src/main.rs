use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use futures::future::join_all;
use regex::Regex;
use serde::Deserialize;
use sm2::{
    dsa::{Signature, SigningKey}, 
};
use std::time::Instant;
use std::{env, fs, sync::Arc};
use sm2::dsa::signature::Signer;
use num_bigint::BigUint;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    business_id: String,
    api_url: String,
    template_code: String,
    key_file_path: String,
}

// 全局日志写入函数，只有在调试模式下才写入文件
static mut DEBUG_MODE: bool = false;

fn set_debug_mode(debug: bool) {
    unsafe {
        DEBUG_MODE = debug;
    }
}

fn is_debug_mode() -> bool {
    unsafe { DEBUG_MODE }
}

fn log_to_file(message: &str) {
    if is_debug_mode() {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let log_entry = format!("[{}] {}\n", timestamp, message);
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("sms_sender.log")
            .expect("无法打开日志文件");
            
        file.write_all(log_entry.as_bytes()).expect("写入日志失败");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数
    let args: Vec<String> = env::args().collect();
    
    // 检查是否有 --debug 参数
    let debug_mode = args.contains(&"--debug".to_string());
    set_debug_mode(debug_mode);
    
    // 找到实际参数的位置（跳过 --debug 参数）
    let actual_args: Vec<String> = if debug_mode {
        args.iter()
            .filter(|arg| *arg != "--debug")
            .cloned()
            .collect()
    } else {
        args.clone()
    };
    
    if actual_args.len() < 3 {
        eprintln!("用法: {} [--debug] <消息内容> <手机号1> <手机号2> ...", actual_args[0]);
        if debug_mode {
            log_to_file(&format!("错误: 用法: {} [--debug] <消息内容> <手机号1> <手机号2> ...", actual_args[0]));
        }
        return Ok(());
    }

    if debug_mode {
        log_to_file("=== 短信发送程序启动 (调试模式) ===");
    }

    // 1. 加载配置
    let config_data = fs::read_to_string("config.json")
        .map_err(|_| "找不到配置文件 config.json")?;
    let config: AppConfig = serde_json::from_str(&config_data)
        .map_err(|e| format!("配置文件格式错误: {}", e))?;
    let config = Arc::new(config);

    // 2. 解析命令行参数（排除 --debug）
    let original_msg = &actual_args[1];
    let filtered_msg = mask_and_format_text(original_msg);
    let message_content = Arc::new(filtered_msg);
    let mobile_list: Vec<String> = actual_args[2..].to_vec();

    if debug_mode {
        log_to_file(&format!("待发送手机号数量: {}", mobile_list.len()));
    }

    // 3. 加载私钥原始字节 (32字节)
    let sk_bytes = load_private_key_bytes(&config.key_file_path)?;
    let sk_bytes_arc = Arc::new(sk_bytes);

    // 4. 初始化 HTTP 客户端
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30)) // 增加超时时间
            .build()?,
    );

    println!("开始并发发送短信任务...");
    if debug_mode {
        log_to_file("开始并发发送短信任务...");
    }

    let start_time = Instant::now();

    // 5. 循环创建并发任务
    let mut tasks = Vec::new();
    for mobile in mobile_list {
        let client = Arc::clone(&client);
        let msg = Arc::clone(&message_content);
        let cfg = Arc::clone(&config);
        let sk_bytes = Arc::clone(&sk_bytes_arc);

        let task = tokio::spawn(async move {
            let task_start = Instant::now();
            match send_single_sms(client, &mobile, &msg, &sk_bytes, cfg).await {
                Ok(_) => {
                    let duration = task_start.elapsed();
                    let success_msg = format!("任务耗时: {:.3}s", duration.as_secs_f64());
                    println!("{}", success_msg);
                    if is_debug_mode() {
                        log_to_file(&format!("成功发送短信到 {}: {}", mobile, success_msg));
                    }
                },
                Err(e) => {
                    let duration = task_start.elapsed();
                    let error_msg = format!("任务耗时: {:.3}s, 错误: {}", duration.as_secs_f64(), e);
                    eprintln!("{}", error_msg);
                    if is_debug_mode() {
                        log_to_file(&format!("发送短信到 {} 失败: {}", mobile, error_msg));
                    }
                }
            }
        });
        tasks.push(task);
    }

    join_all(tasks).await;

    let total_duration = start_time.elapsed();
    let total_msg = format!("所有任务处理完毕，总耗时: {:.3}s", total_duration.as_secs_f64());
    println!("{}", total_msg);
    if debug_mode {
        log_to_file(&total_msg);
        log_to_file("=== 短信发送程序结束 ===");
    }

    Ok(())
}

/// 将SM2签名转换为DER格式
fn signature_to_der_format(signature: &Signature) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let signature_bytes = signature.to_bytes();
    
    // SM2签名包含r和s两个32字节的大整数
    if signature_bytes.len() != 64 {
        return Err("Invalid signature length".into());
    }
    
    let r_bytes = &signature_bytes[0..32];
    let s_bytes = &signature_bytes[32..64];
    
    // 将字节转换为大整数
    let r = BigUint::from_bytes_be(r_bytes);
    let s = BigUint::from_bytes_be(s_bytes);
    
    // 创建DER编码的ASN.1序列
    // SEQUENCE { INTEGER(r), INTEGER(s) }
    let mut result = vec![0x30]; // SEQUENCE tag
    
    // 编码r和s为INTEGER类型
    let r_encoded = encode_asn1_integer(&r.to_bytes_be());
    let s_encoded = encode_asn1_integer(&s.to_bytes_be());
    
    let content_len = r_encoded.len() + s_encoded.len();
    
    // 添加长度编码
    if content_len < 128 {
        result.push(content_len as u8);
    } else {
        let len_bytes = encode_length(content_len);
        result.extend_from_slice(&len_bytes);
    }
    
    result.extend_from_slice(&r_encoded);
    result.extend_from_slice(&s_encoded);
    
    Ok(result)
}

fn encode_asn1_integer(bytes: &[u8]) -> Vec<u8> {
    let mut result = vec![0x02]; // INTEGER tag
    
    // 如果最高位是1，需要在前面添加0x00以确保为正数
    let mut data = bytes.to_vec();
    if !data.is_empty() && data[0] & 0x80 != 0 {
        data.insert(0, 0x00);
    }
    
    let len = data.len();
    if len < 128 {
        result.push(len as u8);
    } else {
        let len_bytes = encode_length(len);
        result.extend_from_slice(&len_bytes);
    }
    
    result.extend_from_slice(&data);
    result
}

fn encode_length(length: usize) -> Vec<u8> {
    let mut len_bytes = Vec::new();
    let mut temp_len = length;
    while temp_len > 0 {
        len_bytes.insert(0, (temp_len & 0xFF) as u8);
        temp_len >>= 8;
    }
    
    let mut result = vec![0x80 | (len_bytes.len() as u8)];
    result.extend_from_slice(&len_bytes);
    result
}

/// 单个短信发送逻辑
async fn send_single_sms(
    client: Arc<reqwest::Client>,
    mobile: &str,
    message: &str,
    sk_bytes: &[u8],
    config: Arc<AppConfig>,
) -> Result<(), String> {
    let log_prefix = format!("手机号: {}", mobile);
    
    if is_debug_mode() {
        log_to_file(&format!("{} 开始发送短信", log_prefix));
    }

    // 使用默认的distid，这是一个字符串
    let distid = "1234567812345678"; // 这是一个示例distid，根据实际情况调整
    let signing_key = SigningKey::from_slice(distid, sk_bytes)
        .map_err(|e| {
            let err_msg = format!("私钥构造失败 [{}]: {:?}", mobile, e);
            if is_debug_mode() {
                log_to_file(&err_msg);
            }
            err_msg
        })?;
    
    let timestamp = Utc::now().timestamp_millis();

    let data_str = format!(
        r#"{{"mobileNo":"{}","templateCode":"{}","params":{{"message":"{}"}},"callBack":false}}"#,
        mobile, config.template_code, message
    );

    let sign_source_str = format!(
        r#"{{"businessId":"{}","timestamp":{},"data":{}}}"#, 
        config.business_id, timestamp, data_str
    );

    // 直接对原始字符串进行签名，不进行SM3摘要
    let sign_bytes = sign_source_str.as_bytes();
    
    // 使用sm2进行签名
    let signature_result = signing_key.try_sign(sign_bytes);
    let signature: Signature = signature_result.map_err(|e| {
        let err_msg = format!("签名失败: {:?}", e);
        if is_debug_mode() {
            log_to_file(&format!("{} {}", log_prefix, err_msg));
        }
        err_msg
    })?;

    // 将签名转换为DER格式，以便与Java BC库兼容
    let signature_der = signature_to_der_format(&signature)
        .map_err(|e| {
            let err_msg = format!("签名转DER失败: {:?}", e);
            if is_debug_mode() {
                log_to_file(&format!("{} {}", log_prefix, err_msg));
            }
            err_msg
        })?;
    let signature_hex = hex::encode(signature_der).to_uppercase();

    let body_str = format!(
        r#"{{"businessId":"{}","data":{},"timestamp":{},"signature":"{}"}}"#,
        config.business_id, data_str, timestamp, signature_hex
    );

    // 打印完整的请求体
    println!("请求体: {}", body_str);
    if is_debug_mode() {
        log_to_file(&format!("{} 请求体: {}", log_prefix, body_str));
    }

    let response = client.post(&config.api_url)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("网络错误: {}", e);
            if is_debug_mode() {
                log_to_file(&format!("{} {}", log_prefix, err_msg));
            }
            err_msg
        })?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if status.is_success() {
        println!("成功: {}", mobile);
        if is_debug_mode() {
            log_to_file(&format!("{} 发送成功", log_prefix));
        }
        Ok(())
    } else {
        let err_msg = format!("失败 [{}]: {} - {}", mobile, status, text);
        if is_debug_mode() {
            log_to_file(&format!("{} {}", log_prefix, err_msg));
        }
        Err(err_msg)
    }
}

/// 解析 PEM 获取 32 字节私钥
fn load_private_key_bytes(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if is_debug_mode() {
        log_to_file(&format!("加载私钥文件: {}", path));
    }
    
    let content = fs::read_to_string(path)?;
    let b64_content = content
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>()
        .replace(" ", "");

    let der_bytes = general_purpose::STANDARD.decode(b64_content)?;

    // 搜索 PKCS#8 私钥标记 [04, 20]
    if let Some(pos) = der_bytes.windows(2).position(|w| w == [0x04, 0x20]) {
        let start = pos + 2;
        if der_bytes.len() >= start + 32 {
            if is_debug_mode() {
                log_to_file("从PKCS#8格式成功提取私钥");
            }
            return Ok(der_bytes[start..start + 32].to_vec());
        }
    }
    
    // 兜底：取最后32字节
    if der_bytes.len() >= 32 {
        if is_debug_mode() {
            log_to_file("使用兜底策略提取私钥");
        }
        Ok(der_bytes[der_bytes.len() - 32..].to_vec())
    } else {
        let err_msg = "私钥长度不足 32 字节";
        if is_debug_mode() {
            log_to_file(err_msg);
        }
        Err(err_msg.into())
    }
}

/// 文本处理：IPv4 脱敏 + 换行符处理
fn mask_and_format_text(text: &str) -> String {
    // 首先处理换行符：替换为指定字符，避免API返回400错误
    let text_with_line_breaks_replaced = text.replace("\n", "\\n").replace("\r", "\\r");
    
    // 然后进行IP脱敏
    let re = Regex::new(r"(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})").unwrap();
    let result = re.replace_all(&text_with_line_breaks_replaced, "$3.$4").to_string();
    
    if text != result {
        if is_debug_mode() {
            log_to_file("检测到IP地址并进行了脱敏处理");
        }
    }
    
    result
}
