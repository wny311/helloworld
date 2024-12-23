use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;
use std::env;
use std::time::{Instant, Duration};

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <mess> <userlist>", args[0]);
        return;
    }

    let mess = &args[1];
    let userlist: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();

    let url = "http://10.11.26.115:8080/api/aiops/message/template/pushSMS";
    //println!("Requesting URL: {}", url);
    //println!("Parameters:");
    //println!("  userList: {:?}", userlist);
    //println!("  mess: {}", mess);
    //println!("  ip: 10.11.26.210");
    //println!("  plateformNo: zabbix");
    //println!("  port: 0");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3)) // 设置超时时间为3秒
        .build()
        .expect("Failed to build client");
    
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let body = json!({
        "userList": userlist,
        "mess": mess,
        "ip": "10.11.26.210",
        "plateformNo": "zabbix",
        "port": "0",
    });
    println!("{}",body.to_string());
    let request = client.post(url).headers(headers.clone()).json(&body);
    println!("Request object: {:?}", request);

    match request.send().await {
        Ok(response) => {
            println!("Response: {:?}", response.text().await.unwrap());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    let duration = start_time.elapsed();
    println!("Execution time: {:?}", duration);
}
