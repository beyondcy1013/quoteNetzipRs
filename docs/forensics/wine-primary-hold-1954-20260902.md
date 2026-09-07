# Wine 主站保持 3 分钟以上：登录股票主站=1 + 跳过旧式 Ask

目录：diagnostics/20260902-live-pair/init-none-primary1-1952/
时间：2026-09-02 19:54:13 主站登录成功 起，连续观察至 19:58:12
配置：用户/配置文件.ini `登录股票主站=1`；QUOTENETZIPWINE_INIT_LOGIN_MODULES=none
事件：初始化完成(sync 70) → 股票主站登录成功 → 无 Ask returned 0、无断开
连接：10 条 ESTAB 58.16.134.228:5188（网际风 fd 215–224），237s pcap 2756 包零 FIN/RST
effective：login_auth=true, login_primary=true, login_backup=false
对照失败：ask-diagnose-1931 同账号同配置但默认旧式 Ask → 主站 6s 后断开
结论：旧式同步 Ask(模块=认证) 是主站被断开/回写 登录股票主站=0 的根因。
