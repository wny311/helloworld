package main

import (
	"encoding/json"
	"fmt"
	"log"
	"os"

	openapi "github.com/alibabacloud-go/darabonba-openapi/v2/client"
	dyvmsapi20170525 "github.com/alibabacloud-go/dyvmsapi-20170525/v6/client"
	util "github.com/alibabacloud-go/tea-utils/v2/service"
	"github.com/alibabacloud-go/tea/tea"
	credential "github.com/aliyun/credentials-go/credentials"
	"github.com/joho/godotenv"
)

/**
 * 使用凭据初始化账号 Client
 * 修复点：显式检查环境变量，避免返回 nil client
 */
func CreateClient() (*dyvmsapi20170525.Client, error) {
	accessKeyId := os.Getenv("ALIYUN_ACCESS_KEY_ID")
	accessKeySecret := os.Getenv("ALIYUN_ACCESS_KEY_SECRET")

	if accessKeyId == "" || accessKeySecret == "" {
		return nil, fmt.Errorf("error: ALIYUN_ACCESS_KEY_ID or ALIYUN_ACCESS_KEY_SECRET is not set in environment")
	}

	config := &openapi.Config{
		AccessKeyId:     tea.String(accessKeyId),
		AccessKeySecret: tea.String(accessKeySecret),

		Endpoint: tea.String("dyvmsapi.aliyuncs.com"),
	}

	credentialsConfig := new(credential.Config).
		SetType("access_key").
		SetAccessKeyId(accessKeyId).
		SetAccessKeySecret(accessKeySecret)

	akCredential, err := credential.NewCredential(credentialsConfig)
	if err != nil {
		return nil, err
	}
	config.Credential = akCredential

	return dyvmsapi20170525.NewClient(config)
}

func _main(args []*string) error {
	client, err := CreateClient()
	if err != nil {
		return err
	}

	// 从命令行参数获取手机号和系统名
	argsList := tea.StringSlice(os.Args[1:])

	// 检查参数数量
	if len(argsList) < 2 {
		return fmt.Errorf("usage: program <phone_number> <system_name>")
	}

	phoneNumber := tea.StringValue(argsList[0])
	systemName := tea.StringValue(argsList[1])

	// 构造请求参数
	singleCallByTtsRequest := &dyvmsapi20170525.SingleCallByTtsRequest{
		CalledNumber: tea.String(phoneNumber), // 使用传入的手机号
		TtsCode:      tea.String("TTS_328550373"),
		TtsParam:     tea.String(fmt.Sprintf(`{"system_name":"%s"}`, systemName)), // 使用传入的系统名
	}

	runtime := &util.RuntimeOptions{}

	// 使用匿名函数处理业务逻辑与异常恢复
	tryErr := func() (e error) {
		defer func() {
			if r := tea.Recover(recover()); r != nil {
				e = fmt.Errorf("tea panic: %v", r)
			}
		}()

		fmt.Println("[INFO] 正在发起语音通知请求...")
		resp, sdkErr := client.SingleCallByTtsWithOptions(singleCallByTtsRequest, runtime)
		if sdkErr != nil {
			return sdkErr
		}

		fmt.Printf("[LOG] 接口调用成功，响应结果: %v\n", resp)
		return nil
	}()

	// 增强错误诊断逻辑
	if tryErr != nil {
		if sdkErr, ok := tryErr.(*tea.SDKError); ok {
			fmt.Printf("[ERROR] SDK Error Message: %s\n", tea.StringValue(sdkErr.Message))

			// 尝试解析并打印诊断地址（Recommend）
			var data map[string]interface{}
			if err := json.Unmarshal([]byte(tea.StringValue(sdkErr.Data)), &data); err == nil {
				if recommend, ok := data["Recommend"]; ok {
					fmt.Printf("[DIAGNOSE] 诊断建议地址: %v\n", recommend)
				}
			}
		} else {
			fmt.Printf("[ERROR] 运行时错误: %v\n", tryErr)
		}
	}

	return nil
}

func main() {
	// 尝试加载 .env 文件
	err := godotenv.Load()
	if err != nil {
		log.Println("Warning: .env file not found, will use system environment variables")
	}
	err = _main(tea.StringSlice(os.Args[1:]))
	if err != nil {
		fmt.Printf("[FATAL] 程序异常退出: %v\n", err)
		os.Exit(1)
	}
}
