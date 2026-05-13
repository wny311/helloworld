package main

import (
	"fmt"
	"net/http"
	"os"

	openapi "github.com/alibabacloud-go/darabonba-openapi/v2/client"
	dyvmsapi20170525 "github.com/alibabacloud-go/dyvmsapi-20170525/v6/client"
	util "github.com/alibabacloud-go/tea-utils/v2/service"
	"github.com/alibabacloud-go/tea/tea"
	credential "github.com/aliyun/credentials-go/credentials"
	"github.com/gin-gonic/gin"
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

// 语音通知请求结构体
type CallRequest struct {
	PhoneNumber string `json:"phone_number" binding:"required"`
	SystemName  string `json:"system_name" binding:"required"`
}

// 语音通知响应结构体
type CallResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
	Code    string `json:"code,omitempty"`
}

// 处理语音通知的主函数
func makeVoiceCall(phoneNumber, systemName string) error {
	client, err := CreateClient()
	if err != nil {
		return err
	}

	ttsCode := os.Getenv("TTS_CODE")

	// 构造请求参数
	singleCallByTtsRequest := &dyvmsapi20170525.SingleCallByTtsRequest{
		CalledNumber: tea.String(phoneNumber),                                     // 使用传入的手机号
		TtsCode:      tea.String(ttsCode),                                         // 使用环境变量中的 TTS_CODE
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
		} else {
			fmt.Printf("[ERROR] 运行时错误: %v\n", tryErr)
		}
		return tryErr
	}

	return nil
}

// HTTP 处理函数
func voiceCallHandler(c *gin.Context) {
	var req CallRequest

	// Gin 自动绑定 JSON 请求体到结构体，并验证必填字段
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{
			"success": false,
			"message": fmt.Sprintf("Invalid request format: %v", err),
		})
		return
	}

	// 执行语音通知
	err := makeVoiceCall(req.PhoneNumber, req.SystemName)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{
			"success": false,
			"message": fmt.Sprintf("Failed to make voice call: %v", err),
		})
		return
	}

	// 成功响应
	c.JSON(http.StatusOK, gin.H{
		"success": true,
		"message": "Voice call initiated successfully",
	})
}

// 健康检查端点
func healthCheckHandler(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"status":  "healthy",
		"service": "voice-call-webhook-server",
	})
}

func main() {
	// 尝试加载 .env 文件
	err := godotenv.Load()
	if err != nil {
		fmt.Println("Warning: .env file not found, will use system environment variables")
	}

	// 设置 Gin 模式
	gin.SetMode(gin.ReleaseMode) // 生产环境使用 Release 模式
	r := gin.Default()

	// 注册路由
	r.POST("/webhook/voice-call", voiceCallHandler)
	r.GET("/health", healthCheckHandler)

	// 启动服务器
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080" // 默认端口
	}

	fmt.Printf("Starting webhook server on port %s...\n", port)
	fmt.Printf("Voice call endpoint: POST /webhook/voice-call\n")
	fmt.Printf("Health check endpoint: GET /health\n")

	// 运行服务器
	r.Run(":" + port)
}
