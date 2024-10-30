# :family: helloworld

## :couple: 个人笔记
## :memo: windows terminal
windows terminal + ohmyzsh + [windows terminal themes](https://windowsterminalthemes.dev/) + [gui generator](https://www.guidgen.com/)

## :memo: vim 清除高亮 
```
:noh
```

## :memo: 1123
use vps(vultr) to sync gcr.io images


## :memo: grafana dashboard
[云原生devops](https://github.com/starsliao/Prometheus)

## :memo: update RHEL kernel grub boot
- 查看当前默认启动内核
```
grub2-editenv list
```
- 查看当前有几个内核
```
cat /boot/grub2/grub.cfg | grep menuentry
```
- 更改默认启动内核
```
grub2-set-default "CentOS Linux (4.4.248-1.el7.elrepo.x86_64) 7 (Core)"
```
- 修改后查看当前默认启动内核
```
grub2-editenv list
```
```
sudo grub2-mkconfig -o /boot/grub2/grub.cfg
```

> gaoliangxianshi
