using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Runtime.InteropServices;
using System.IO;
using System.Threading;
using System.Windows.Forms;

//总体说明：调用很简单，载入dll→发出指令→等待结果→解析结果
//需要使用的文件：Stock.dll + Stock.dat + 用户目录（配置文件.ini + 服务器列表.json）
//StockUp.ini，升级配置文件。
namespace StockCS
{
    class Test
    {
        [DllImport("Stock.dat")]
        private extern static int Start(RcvCallback_ callBack); //注册回调函数

        [DllImport("Stock.dat")]
        private extern static int Ask([MarshalAs(UnmanagedType.LPTStr)] string ask, IntPtr answer, int maxLen); //发出指令，参数参考调用规范填写，比如 下单?请求=登录&券商=信达证券
        [DllImport("Stock.dat")]
        private extern static int Stop();   //注销

        [UnmanagedFunctionPointerAttribute(CallingConvention.StdCall, CharSet = CharSet.Unicode)]
        public delegate int RcvCallback_([MarshalAs(UnmanagedType.LPTStr)] String form, IntPtr answer);  //定义回调函数对应的委托

        public static RcvCallback_ m_callBack = null;
        public static int g_tick = System.Environment.TickCount - 100000;

        //定义自己的回调函数
        //调用机制：用户发出指令，通过网络发往服务器进行交易，结果通过回调函数返回
        //回调和发出指令是独立的，发出指令后，不需要等待处理结果。
        //主程序需要等待回调函数处理完成，请将结果拷贝到内存后，直接返回。
        public static int Answer([MarshalAs(UnmanagedType.LPTStr)] String form, IntPtr answer)		//answer 返回缓冲区；form 辅助说明
        {
            string ans = Marshal.PtrToStringUni(answer);
            if (form == "错误")
            {
                if (ans.IndexOf("dll升级后需要重新载入") >= 0)
                {
                    MessageBox.Show("Stock.dll 有升级，需要重启程序后生效。", "提示信息", MessageBoxButtons.YesNo);

                    return 0;    //返回 0 ，然后重启您的程序。
                }
                Console.Write(ans);
                Console.Write("\r\n");

                return 1;
            }
            if (form == "股票数据")
            {
                DoStock(answer);
            }
            else if (form == "无效请求")
            {
                Console.Write(ans);
                Console.Write("\r\n");
            }
            else if (form == "提示信息")
            {
                Console.Write(ans);
                Console.Write("\r\n");
            }
            else
            {
                Console.Write(ans);
                Console.Write("\r\n");
            }

            return 1;
        }

        static void Main(string[] args)
        {
            Console.Write("股票数据诊断模式。\r\n\r\n");

            Console.Write("按 q + 回车 退出演示。\r\n\r\n");
            Console.Write("正在初始化客户端。\r\n\r\n");
            Console.Write("这里直接粘贴调用指令。\r\n\r\n");

            m_callBack = Answer;

            int maxLen = 20 * 1000 * 1024;                 //20M预留空间，用于指令带 “&等待=毫秒数”调用方式下保存请求结果。异步方式（没有带等待），忽略。
            IntPtr answer = Marshal.AllocHGlobal(maxLen);  //多线程，需要每个线程分配一次
            string ask = null;
            if (Start(m_callBack) == 0)                       //注册回调函数
            {
                Console.Write("初始化失败，但忽略并继续。\r\n\r\n");
                // ask = Console.ReadLine();
                // return;
            }
            int ret = Ask("股票数据?请求=登录&模块=认证&账号=&密码=&自动升级=稳定版&版本=20221120&等待=10000&编号=0", answer, maxLen);
            if (ret > 0)
            {
                DoStock(answer);  //“&等待=毫秒数”，请求后等待服务器返回结果。
                                  //“&等待=0”，     请求后通过回调函数返回。
            }
            ret = Ask("股票数据?请求=登录&模块=股票备用&账号=&密码=&等待=10000&编号=20", answer, maxLen);
            if (ret > 0)
            {
                DoStock(answer);
            }
            ret = Ask("股票数据?请求=初始化&分析软件=自定义&等待=10000&编号=-1", answer, maxLen);
            if (ret > 0)
            {
                DoStock(answer);
            }
            while (true)
            {
                ask = Console.ReadLine();
                if (ask == null)                  //非控制台方式
                {
                    break;
                }
                 if (ask == "q")  
                {
                    break;
                }
                ret = Ask(ask, answer, maxLen);  //输入调用指令
                if (ret > 0)
                {
                    g_tick = System.Environment.TickCount - 100000;
                    DoStock(answer);
                }
            }
            Console.Write("正在退出……\r\n\r\n");
            Stop();                                 //注销

            Marshal.FreeHGlobal(answer);            //释放内存
        }
        public static int DoStock(IntPtr answer)       //网际风多线程推送数据，请在 0.01秒内处理完成，建议拷贝到内存，由其它线程处理。
        {
            OEM_DATA_HEAD head = Marshal.PtrToStructure<OEM_DATA_HEAD>(answer);  //net 4.5.1 增加新方法
            int count = head.count;
            int headLen = Marshal.SizeOf<OEM_DATA_HEAD>();    //200字节
            IntPtr data = answer + headLen;
           if (head.type == "代码表")
            {
                int size = Marshal.SizeOf<OEM_MARKETINFO>();    //200字节
                OEM_MARKETINFO market = Marshal.PtrToStructure<OEM_MARKETINFO>(data);
                OEM_STKINFO stkInfo = Marshal.PtrToStructure<OEM_STKINFO>(data + size);
                int num = market.num;
                data += size;
                int node = Marshal.SizeOf<OEM_STKINFO>();    //250字节
                for (int i = 0; i < num; i++)
                {
                    stkInfo = Marshal.PtrToStructure<OEM_STKINFO>(data + i * node);
                    if (i < 2)
                    {
                        Console.WriteLine(market.name + "\t" + stkInfo.label + "\t" + stkInfo.name);
                    }
                }

                return 1;
            }
            if (head.type == "实时数据")
            {
                OEM_REPORT report;
                int size = Marshal.SizeOf<OEM_REPORT>();    //500字节
                for (int i = 0; i < count; i++)
                {
                    report = Marshal.PtrToStructure<OEM_REPORT>(data + i * size);

                    if (report.label == "SH600000")
                    {
                        int tick = System.Environment.TickCount;
                        if (tick - g_tick > 60000)  //60 秒刷新
                        {
                            g_tick = tick;
                            string str = "接收到实时数据" + report.label + "(" + report.name + ")" + " 现价：" + report.close + " 时间：" + report.Time;

                            Console.WriteLine(str);
                        }
                    }
                }

                return 1;
            }
            if (head.type == "分笔")
            {
                OEM_TICK tick;
                int size = Marshal.SizeOf<OEM_TICK>();    //100字节
                for (int i = 0; i < count; i++)
                {
                    tick = Marshal.PtrToStructure<OEM_TICK>(data + i * size);

                }
                if (head.label == "SH600000")
                {
                    string str = "接收到分笔数据：" + head.label;

                    Console.WriteLine(str);
                }

                return 1;
            }
            if (Kline(head.type))
            {
                OEM_KLINE kline;
                int size = Marshal.SizeOf<OEM_KLINE>();    //32字节
                for (int i = 0; i < count; i++)
                {
                    kline = Marshal.PtrToStructure<OEM_KLINE>(data + i * size);
                }
                string str = "接收到K线数据：" + head.label + head.type + "\t数量:" + count + "\r\n";
                Console.WriteLine(str);
                if (head.label == "SH600000")
                {
                    for (int i = 0; i < count; i++)
                    {
                        kline = Marshal.PtrToStructure<OEM_KLINE>(data + i * size);
                        str = "\t时间：" + kline.Time + "\t现价：" + kline.close;

                        Console.WriteLine(str);
                    }
                }

                return 1;
            }
            if (head.type == "除权")   //头部 + 实际内容
            {
                OEM_SPLIT split;
                OEM_SPLIT_HEAD sHead;
                int size = Marshal.SizeOf<OEM_SPLIT_HEAD>();    //200字节
                int size2 = Marshal.SizeOf<OEM_SPLIT>();        //200字节
                for (int i = 0; i < count;)
                {
                    sHead = Marshal.PtrToStructure<OEM_SPLIT_HEAD>(data + i * size);     //取出头部
                    int num = sHead.num;
                    i++;
                    for (int j = 0; j < num; j++)
                    {
                        split = Marshal.PtrToStructure<OEM_SPLIT>(data + (i + j) * size);
                    }
                    i += num;
                    if (sHead.label == "SH600000")
                    {
                        string str = "接收到除权数据：" + sHead.label + "\t" + sHead.name + "\r\n";

                        Console.WriteLine(str);
                    }
                }

                return 1;
            }
            if (head.type == "财务")
            {
                OEM_FINANCE finance;
                int size = Marshal.SizeOf<OEM_FINANCE>();    //350字节
                for (int i = 0; i < count; i++)
                {
                    finance = Marshal.PtrToStructure<OEM_FINANCE>(data + i * size);

                    if (finance.label == "SH600000")
                    {
                        string str = "接收到财务数据：" + finance.label;

                        Console.WriteLine(str);
                    }
                }

                return 1;
            }
            if (head.type == "F10资料")                  //代码 + 名称 + 索引 + 文本，是否有更高效的处理方式？
            {
                OEM_F10INFO f10;
                int size = Marshal.SizeOf<OEM_F10INFO>();    //50字节
                int hLen = head.value[0];
                string sub = "";
                string txt = Marshal.PtrToStringUni(data + hLen);
                for (int i = 0; i < count; i++)
                {
                    f10 = Marshal.PtrToStructure<OEM_F10INFO>(data + i * size);
                    sub = txt.Substring(f10.from, f10.len);
                }

                if (head.label == "SH600000")
                {
                    string str = "接收到F10资料：" + head.label + "\r\n";
                    Console.WriteLine(str);
                }

                return 1;
            }
            if (head.type == "6到10档挂单")
            {
                OEM_BUYSELL6_10 r = Marshal.PtrToStructure<OEM_BUYSELL6_10>(data);
                if (head.label == "SH600000")
                {
                    string str = "接收到 " + head.label + "6到10档挂单" + "\r\n\r\n";
                    Console.WriteLine(str);
                }

                return 1;
            }

            string ans = Marshal.PtrToStringUni(answer);
            Console.Write(ans);
            Console.Write("\r\n");

            return 0;
        }
        private static bool Kline(string type)
        {
            string[] arr = { "1分钟线", "5分钟线", "15分钟线", "30分钟线", "60分钟线", "日线", "周线", "月线", "季线", "年线", "多日线" };

            int first = Array.IndexOf(arr, type);

            return first >= 0;
        }
    }
}
