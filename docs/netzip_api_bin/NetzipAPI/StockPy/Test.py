

#!委托下单模块示例(python 3.9 32位)

#需要安装pywin32模块，pip install pywin32

#总体说明：调用很简单，载入dll→发出指令→等待结果→解析结果
#需要使用的文件：Stock.dll + Stock.exe + 用户目录（配置文件.ini + 服务器列表.ini）
#StockUp.ini，升级配置文件。

import ctypes
import struct
import time
import os

from OemStock import *

def Kline(type):  # 是否K线格式
    arr = ["1分钟线", "5分钟线", "15分钟线", "30分钟线", "60分钟线", "日线", "周线", "月线", "季线", "年线", "多日线"]
    x = arr.count(type)
    return x
   
def DoStock(answer):  # 实时全推数据处理量大，建议拷贝数据后，扔到队列去 其它线程去处理
    p_head = cast(answer, POINTER(OEM_DATA_HEAD))
    head  = p_head[0]
    count = head.count
    kind  = head.type
    size  = sizeof(OEM_DATA_HEAD)  # 100 字节

    data = byref(head, size)
    if kind == '代码表':
        # size = sizeof(OEM_STKINFO)
        p_market = cast(data, POINTER(OEM_MARKETINFO))
        market = p_market[0]
        stkInfo = cast(market.stkInfo, POINTER(OEM_STKINFO))  # 转换为指针，方便后续读入
        num = market.num
        mkName = market.name
        for i in range(num):
            s = stkInfo[i]
            label = s.label
            name = s.name
            if i < 2:
                print("%s.%s(%s)" % (mkName, label, name))

        return 1
    elif kind == '实时数据':
        size = sizeof(OEM_REPORT)
        pReport = cast(data, POINTER(OEM_REPORT))
        for i in range(count):
            report = pReport[i]
            if report.label == 'SH600000':
                label = report.label
                name = report.name
                tm = time.localtime(report.time)
                tm_str = time.strftime('%Y-%m-%d %H:%M:%S', tm)
                print("\r\n接收到实时数据%s(%s) : time:%s close : %0.2f\r\n" % (label, name, tm_str, report.close))
        return 1
    elif kind == '分笔':
        # size = sizeof(OEM_TICK)
        pTrace = cast(data, POINTER(OEM_TICK))
        buf = create_string_buffer(head.len + 1)
        memmove(buf, pTrace, head.len)  # 拷贝到内存

        if head.label == 'SH600000':
            label = head.label
            name = head.name
            print("接收到分时/分笔数据%s(%s) : " % (label, name))
        return 1
    elif Kline(kind):
        size   = sizeof(OEM_KLINE)
        pKline = cast(data, POINTER(OEM_KLINE))
        #buf    = create_string_buffer(head.len + 1)
        #memmove(buf, pKline, head.len)  # 直接拷贝到内存，然后保存，有需要到内存读取

        # 下面的解析非常慢，是否有其它方式更快遍历数据？
        for i in range(count):
            kline = pKline[i]

        label = head.label
        name = head.name
        print("接收到K线(%s)数据%s(%s) 数量 %d : " % (kind, label, name, count))
        return 1
    elif kind == '除权':  # 头部 + 实际内容
        sHead = cast(data, POINTER(OEM_SPLIT_HEAD))
        split = cast(data, POINTER(OEM_SPLIT))
        i = 0
        while i < count:
            h   = sHead[i]  # 取出头部
            num = h.num
            if h.label == 'SH600000':
                label = h.label
                name = h.name
                print("\r\n接收到除权数据%s(%s) : \r\n" % (label, name))
            i += 1
            for j in range(num):
                s = split[i + j]
            i += num

        return 1
    elif kind == '财务':
        pFin = cast(data, POINTER(RCV_FINANCE))
        for i in range(count):
            f = pFin[i]
            if f.label == 'SH600000':
                label = f.label
                name = f.name
                print("\r\n接收到财务数据%s(%s) : \r\n" % (label, name))

        return 1
    elif kind == 'F10资料':  # 代码 + 名称 + 索引 + 文本，是否有更高效的处理方式？
        size = sizeof(OEM_F10INFO)
        h_len = count * size
        h = cast(data, POINTER(OEM_F10INFO))  # 索引
        t = byref(h[0], h_len)
        txt = cast(t, ctypes.c_wchar_p)
        # str = txt.contents.data
        for i in range(count):
            id = h[i]

        if head.label == 'SH600000':
            label = head.label
            name = head.name
            print("\r\n接收到F10数据%s(%s)\r\n" % (label, name))

            # fName = "F10\\" + label + '.f10';  #保存索引备用
            # with open(fName, 'w') as f10:
            #    f10.write(h)

            # fName = "F10\\" + label + '.txt';  #保存文本
            # with open(fName, 'w') as fTxt:
            #    fTxt.write(str)

        return 1
    else:
        print(kind)

#定义自己的回调函数
#回调和发出指令是独立的，发出指令后，不需要等待处理结果。
#主程序需要等待回调函数处理完成，请将结果拷贝到内存后，直接返回。

def Answer(form, answer, askId):    #回调函数：form 返回格式（文本、JSON、二进制）, answer 返回结果；
    str = ctypes.cast(answer, ctypes.c_wchar_p );
    if form == '股票数据':
        DoStock(answer)
        return 1
    elif form == "错误":
        if str.value.find("重新载入 Stock.dll") > 0:
            win32api.MessageBox(0, "Stock.dll 有升级，需要重启程序后生效。", "提示信息", win32con.MB_YESNO)
            return 0    #返回 0 ，然后重启您的程序。
        if str.value == "内存不足关闭程序 Stock.dat":
            return 0    #返回 0 ，然后关闭您的程序。
        return 1
    elif form == "股票数据":
        DoStock(answer)
    elif form == "无效请求":
        print(str.value)
    elif form =="提示信息":
	    print(str.value)
    else:
	    print(str.value)
    return 1

if __name__ == "__main__":
    print("股票接口诊断模式。\r\n\r\n")

    print("按 q + 回车 退出演示。\r\n\r\n")
    print("正在初始化客户端。\r\n\r\n")
    print("参考调用规范，直接输入申请指令。\r\n\r\n")
    path = os.path.abspath('Stock.dll')
    dll  = ctypes.windll.LoadLibrary(path)
    Start  = dll.Start    #注册回调函数。参数 ：需要注册的回调函数
    Ask    = dll.Ask      #发出指令，比如 下单?请求=登录&券商=信达证券。参数 ：指令字符串 + 存放结果缓冲区 + 缓冲区大小(字节)
    Stop   = dll.Stop     #注销

    len    = 10 * 1000 * 1024
    INPUT  = ctypes.c_wchar * len
    answer = INPUT()

    CMPFUNC   = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_int)  #回调函数创建类型
    _callback = CMPFUNC(Answer)
    Start(_callback)                   #初始化
    ask = "股票数据?请求=登录&模块=认证&账号=&密码=&自动升级=稳定版&版本=20221120&等待=10000&编号=0"
    ret = Ask(ask, answer, 2 * len)              
    if ret > 0:
        print(answer.value)            #“&等待=毫秒数”，请求后等待服务器返回结果
                                       #“&等待=0”，     请求后通过回调函数返回
    ask = "股票数据?请求=登录&模块=股票备用&账号=&密码=&等待=10000&编号=20"
    ret = Ask(ask, answer, 2 * len)        
    if ret > 0:
        print(answer.value)

    ask = "股票数据?请求=初始化&分析软件=自定义&等待=10000&编号=-1"
    ret = Ask(ask, answer, 2 * len)        
    if ret > 0:
        print(answer.value)

    while True:
        ask = input()
        if ask == 'q':
            break
        ret = Ask(ask, answer, 2 * len);  #输入下单指令                
        if ret > 0:
            DoStock(answer)           #指令带 “&等待=毫秒数”，请求后等待服务器返回结果，不推荐使用。
                                      #建议使用回调函数方式无需等待，支持连续请求

    print("正在退出……")
    Stop()        #注销
