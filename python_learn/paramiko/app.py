#!/usr/bin/env python3
import paramiko
import csv
import time
import logging
import argparse
import threading
from queue import Queue
from datetime import datetime

# 配置日志系统
def setup_logging():
    logger = logging.getLogger('ssh_executor')
    logger.setLevel(logging.INFO)
    
    # 创建控制台处理器
    console_handler = logging.StreamHandler()
    console_handler.setLevel(logging.INFO)
    
    # 创建文件处理器
    file_handler = logging.FileHandler(f'ssh_executor_{datetime.now().strftime("%Y%m%d_%H%M%S")}.log')
    file_handler.setLevel(logging.DEBUG)
    
    # 创建格式化器
    formatter = logging.Formatter('%(asctime)s - %(threadName)s - %(levelname)s - %(message)s')
    console_handler.setFormatter(formatter)
    file_handler.setFormatter(formatter)
    
    # 添加处理器到日志器
    logger.addHandler(console_handler)
    logger.addHandler(file_handler)
    
    return logger

# 从CSV文件读取服务器配置
def read_server_config(file_path):
    servers = []
    try:
        with open(file_path, 'r') as f:
            reader = csv.reader(f)
            for row in reader:
                if len(row) >= 5:  # 确保有足够的数据（包括su密码）
                    server = {
                        'ip': row[0].strip(),
                        'password': row[1].strip(),
                        'port': int(row[2].strip()) if len(row) > 2 and row[2].strip() else 57758,
                        'username': row[3].strip() if len(row) > 3 and row[3].strip() else 'etc',
                        'su_password': row[4].strip()  # 新增su密码字段
                    }
                    servers.append(server)
        return servers
    except Exception as e:
        logger.error(f"读取配置文件失败: {e}")
        return []

# 在单个服务器上执行命令
def execute_command_on_server(server, command, logger):
    start_time = time.time()
    logger.info(f"开始处理服务器: {server['ip']}")
    
    try:
        # 创建SSH客户端
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        
        # 1. 使用普通用户连接
        logger.debug(f"连接服务器: {server['ip']}:{server['port']} 用户: {server['username']}")
        ssh.connect(
            hostname=server['ip'],
            port=server['port'],
            username=server['username'],
            password=server['password'],
            timeout=30
        )
        logger.info(f"成功连接到服务器: {server['ip']}")
        
        # 2. 创建交互式shell会话
        channel = ssh.invoke_shell()
        time.sleep(1)  # 等待shell初始化
        
        # 3. 发送su命令切换到root
        logger.debug(f"发送su命令切换到root")
        channel.send('su -\n')
        time.sleep(1)  # 等待密码提示
        
        # 发送su密码
        channel.send(server['su_password'] + '\n')  # 使用配置文件中的su密码
        time.sleep(1)  # 等待认证
        
        # 检查是否成功切换到root
        channel.send('whoami\n')
        time.sleep(1)
        output = ""
        while channel.recv_ready():
            output += channel.recv(1024).decode('utf-8', errors='ignore')
        
        if 'root' not in output:
            logger.error(f"切换到root失败: {output}")
            raise Exception("切换到root失败")
        
        logger.info(f"成功切换到root用户")
        
        # 4. 发送要执行的命令
        logger.info(f"执行命令: {command}")
        channel.send(command + '\n')
        time.sleep(2)  # 等待命令执行
        
        # 5. 读取命令输出
        output = ""
        while channel.recv_ready():
            resp = channel.recv(1024).decode('utf-8', errors='ignore')
            output += resp
        
        # 清理输出 - 移除命令回显和提示符
        clean_output = output.replace(command, '').strip()
        clean_output = clean_output.split('\n', 1)[-1]  # 移除第一行（命令回显）
        clean_output = clean_output.rsplit('\n', 1)[0] if '\n' in clean_output else clean_output  # 移除最后一行（提示符）
        
        logger.info(f"命令输出:\n{clean_output}")
        
        # 6. 退出su和SSH会话
        channel.send('exit\n')  # 退出su后的root shell
        time.sleep(0.5)
        channel.send('exit\n')  # 退出普通用户的SSH会话
        time.sleep(0.5)
        
        # 7. 关闭连接
        channel.close()
        ssh.close()
        
        elapsed = time.time() - start_time
        logger.info(f"服务器处理完成: {server['ip']} (耗时: {elapsed:.2f}秒)")
        return True, clean_output
        
    except Exception as e:
        elapsed = time.time() - start_time
        logger.error(f"处理服务器 {server['ip']} 失败: {e} (耗时: {elapsed:.2f}秒)")
        return False, str(e)

# 工作线程函数
def worker(command, queue, logger):
    while not queue.empty():
        server = queue.get()
        logger.info(f"线程 {threading.current_thread().name} 开始处理服务器: {server['ip']}")
        success, output = execute_command_on_server(server, command, logger)
        queue.task_done()

# 主函数
def main():
    # 解析命令行参数
    parser = argparse.ArgumentParser(description='在多台服务器上执行命令')
    parser.add_argument('-f', '--file', required=True, help='服务器配置文件路径 (CSV格式)')
    parser.add_argument('-c', '--command', required=True, help='要在服务器上执行的命令')
    parser.add_argument('-t', '--threads', type=int, default=5, help='并发线程数 (默认: 5)')
    args = parser.parse_args()
    
    global logger
    logger = setup_logging()
    
    logger.info("=" * 80)
    logger.info(f"开始执行批量命令任务")
    logger.info(f"配置文件: {args.file}")
    logger.info(f"执行命令: {args.command}")
    logger.info(f"并发线程数: {args.threads}")
    logger.info("=" * 80)
    
    # 读取服务器配置
    logger.info(f"读取服务器配置文件: {args.file}")
    servers = read_server_config(args.file)
    
    if not servers:
        logger.error("未找到有效的服务器配置，程序退出")
        return
    
    logger.info(f"找到 {len(servers)} 台服务器")
    
    # 创建任务队列
    queue = Queue()
    for server in servers:
        queue.put(server)
    
    # 创建并启动工作线程
    threads = []
    for i in range(args.threads):
        thread = threading.Thread(
            target=worker,
            args=(args.command, queue, logger.getChild(f'Thread-{i+1}')),
            name=f'Worker-{i+1}'
        )
        thread.daemon = True
        thread.start()
        threads.append(thread)
        logger.info(f"启动线程: {thread.name}")
    
    # 等待所有任务完成
    logger.info("等待所有任务完成...")
    queue.join()
    
    logger.info("=" * 80)
    logger.info("所有服务器处理完成")
    logger.info("=" * 80)

if __name__ == "__main__":
    main()
