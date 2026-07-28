using System;
using System.Collections.Generic;
using System.Text;

//股票双向接口调用规范5.0
//文档使用UNICODE编码 微软雅黑字体，11字号
//带 * 表示 系统内部预留，可能不提供或不准确
//更新时间：                2023.04.25  

namespace StockCS
{
    using System.Linq;
    using System.Runtime.InteropServices;
 
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)] //按字节对齐
    public unsafe struct OEM_DATA_HEAD		        //通信头部(200字节)
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 10)]
        public string type;                         //各种子功能号：日线、5分钟线
        public int    len;			                //数据长度
        public int    count;                        //多少组数据
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string label;                        //代码
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string name;                         //名称
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 62)]
        public sbyte[] temp;                       //预留

        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
        public int[]  value;				        //交换某些指定的值 84
        public sbyte  flag;						    //-1设置某个值；0 服务器推送；1 读取本地数据
        public int    askId;				        //申请编号(网际风内部功能)
        public sbyte  power;                        //复权：0 无复权或不填写(默认)；-1 向前复权：历史方向(通常做法)；1 向后复权：未来方向
        public uint   oemVer;                       //版本号

        //接着是存放返回数据
        public void Init()
        {
            this.len   = 0;
            this.count = 0;
        }
        int PackLen()
        {
            return len + Marshal.SizeOf <OEM_DATA_HEAD> ();	//数据 + 头部
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)] //按字节对齐
   public unsafe struct OEM_TEXT		            //文本信息
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 75)]
        public string txt;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    public unsafe struct OEM_STKINFO	           //证券(250字节)
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string label;                       //代码
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string name;                        //名称
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string pinYin;                       //汉字拼音简称
        public ushort code;                         //内部编码
        public byte   market;			            //市场MK_SH;MK_SZ，表明是上海、深圳等市场
        public byte   block;                        //所属系统板块，BK_SHAG，BK_SZAG，表明是上海A股，深圳A股系统板块*
        public sbyte  pointNum;	                    //小数点个数0、1、2、3	
        public ushort hand;                         //每手股数

        public float last;                          //昨收
        public float limitUp;		                //涨停
        public float limitDown;                     //跌停

        public sbyte isIndex;	                    //是否指数*
        public sbyte isDaPan;	                    //是否大盘*
        public sbyte isStock;	                    //股票标识*
        public sbyte bsNum;	                        //几档行情，比如1档、5档
        public short  tmCount;		                //交易时段个数
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)]
        public short[] openTime;		           //开市时间 1,2,3,4,5 (开盘分钟数，比如 0X023A（570）表示 9:30) 
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)]
        public short[] closeTime;		           //收市时间 1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 41)]
        public byte[] temp;                         //预留
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    public unsafe struct OEM_MARKETINFO		       //市场内容(200字节)
    {
        public ushort mkId;				           //市场代码, 'HS', 'SZ', 'JZ', 'HW' ..... 
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public String name;		                   //市场名称(上海交易所)
        public short  tmCount;		               //交易时段个数
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)]
        public short[] openTime;		           //开市时间 1,2,3,4,5 (开盘分钟数，比如 0X023A（570）表示 9:30) 
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)]
        public short[] closeTime;		           //收市时间 1,2,3,4,5
        public uint date;				           //数据日期（201301010）
        public ushort num;                         //该市场的证券个数
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 94)]
        public sbyte[] temp;                       //预留           100
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    public unsafe struct OEM_REPORT  //实时数据，500 字节 带*表示网际风内部数据，请跳过
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string label;            //代码
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string name;             //名称

        public uint time;               //成交时间
        public int foot;                //最后处理分钟数*
        public uint openDate;			//市场开盘日期零点*
        public uint openTime;			//市场开盘时间*
        public uint closeDate;			//市场日线日期零点*

        public float open;              //今日开盘
        public float high;              //今日最高
        public float low;               //今日最低
        public float close;             //最新价格
        public float volume;            //总成交量
        public float amount;            //总成交金额
        public float inVol;			    //内盘*
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] pricesell;	    //申卖价1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] volsell;		    //申卖量1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] vsellCha;        //申卖量变化
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] pricebuy;	    //申买价1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] volbuy;		    //申买量1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] vbuyCha;         //申买量变化*

        public sbyte jingJia;           //集合竞价
        public float avPrice;           //期货的结算价（平均价）
        public sbyte isBuy;             //0 下跌；1 上涨或平盘*
        public float nowv;              //分笔成交量
        public float nowa;              //分笔成交额(期货仓差)


        public float change;            //换手率*
        public float weiBi;             //委比*
        public float liangBi;           //量比*
        public float position;          //期货持仓*
        public float last;              //昨收
        public float limitUp;           //涨停
        public float limitDown;         //跌停
        public sbyte isIndex;	        //是否指数*
        public sbyte isDaPan;	        //是否大盘*
        public sbyte isStock;	        //股票标识*
        public sbyte bsNum;	            //几档行情，比如1档、5档
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 78)]
        public byte[] temp;             //系统预留*
        public DateTime Time
        {
            get { return DateTime.FromFileTimeUtc(time * 10000000L + 116444736000000000).ToLocalTime(); }
        }
        public string Market   //获得市场代码部分
        {
            get
            {
                if (this.label == null || this.label.Length < 2)
                {
                    return string.Empty;
                }
                else
                {
                    return this.label.Substring(0, 2);
                }
            }
        }
        public string Label   //获得代码
        {
            get
            {
                if (this.label == null || this.label.Length <= 2)
                {
                    return string.Empty;
                }
                else
                {
                    return this.label.Substring(2, this.label.Length - 2);
                }
            }
        }
    }
    
    public unsafe struct OEM_BUYSELL6_10    //6到10档挂单
    {
        public float  avBuy;                //挂单买均价
        public float  avSell;               //挂单卖均价
        public float  buyVol;               //挂单买量
        public float  sellVol;              //挂单卖量
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] bsPrice6;                   //6到10档挂单价
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
        public float[] bsVolume6;                   //6到10档挂单量
    }

    public unsafe struct OEM_TICK    //分时或分笔数, 100 字节
    {
        public uint time;                         //时间
        public float close;                       //最新价格
        public float volume;                      //总成交量
        public float amount;                      //总成交金额或期货持仓量
        public int traceNum;                      //分笔数 高位是否内外盘

        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
        public float[] pricebuy;                   //申买价1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
        public float[] volbuy;                    //申买量1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
        public float[] pricesell;                 //申卖价1,2,3,4,5
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
        public float[] volsell;                  //申卖量1,2,3,4,5
        public DateTime Time  //转换很慢，未验证。优化算法：直接算出离1970年天数，如果天数一样，则直接取出年月日时分秒。
        {
            get { return DateTime.FromFileTimeUtc(time * 10000000L + 116444736000000000).ToLocalTime(); }
        }
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public unsafe struct OEM_KLINE    //日线、5分钟、1分钟线结构，32字节
    {
        public uint time;             //时间
        public float open;            //开盘价
        public float high;            //最高价
        public float low;             //最低价
        public float close;           //收盘价
        public float vol;             //成交量
        public float amount;          //成交金额
        public float temp;            //系统预留
        public DateTime Time
        {
            get { return DateTime.FromFileTimeUtc(time * 10000000L + 116444736000000000).ToLocalTime(); }
        }
    };

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    struct OEM_SPLIT_HEAD                 //除权头部 200 字节
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string label;              //代码
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string name;                //名称
        public ushort num;                 //数量
        public uint   volume;              //预留
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 106)]
        public byte[] temp;                //系统预留*
    };

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    public unsafe struct OEM_SPLIT        //200字节
    {
        public uint time;                 //时间
        public float give;                //每股送
        public float allocate;            //每股配多少股
        public float price;               //每股配价
        public float earnings;            //每股红利
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 90)]
        public string explain;            //描述文本(主服务器提供该数据，副服务器没有该项数据)
        public DateTime Time
        {
            get { return DateTime.FromFileTimeUtc(time * 10000000L + 116444736000000000).ToLocalTime(); }
        }
    };

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]     //按字节对齐
    public unsafe struct OEM_FINANCE               //财务数据(350字节)
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string label;                       //代码
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string name;                        //名称

        public int   time;                         //发布日期		年月日
        public int   baoGao;                       //报告日期
        public int   shangShi;                     //上市日期
        public float mgShouYi;                     //每股收益(元)，净利润/总股本
        public float mgJingZhi;                    //每股净资产(元)
        public float jzcsyl;                       //净资产收益率(%)
        public float mgXianJin;                    //每股经营现金
        public float mggjj;                        //每股公积金(元)
        public float mgwfp;                        //每股未分配利润(元)
        public float gdqybl;                       //股东权益比率(%)
        public float jlrtb;                        //净利润同比		10
        public float zysytb;                       //主营收益同比
        public float xsmll;                        //销售毛利率
        public float tzJingZhi;                    //调整每股净资(元)
        public float zongZC;                       //总资产(万元)
        public float ldzc;                         //流动资产(万元)
        public float guDingZC;                     //固定资产(万元)
        public float wuXingZC;                     //无形资产(万元)
        public float ldfuZhai;                     //流动负债(万元)
        public float cqfuZhai;                     //长期负债(万元)
        public float zongfuZhai;                   //总负债	20
        public float quanYi;                       //股东权益(净资产 万元)
        public float zbGongJi;                     //资本公积金(万元)
        public float xianJin;                      //经营现金流量
        public float tzxjl;                        //投资现金流量
        public float czxjl;                        //筹资现金流量
        public float xjzje;                        //现金增加额
        public float shouRu;                       //主营业务收入(万元)
        public float zyLiRun;                      //主营利润(万元)
        public float yyLiRun;                      //营业利润(万元)
        public float tzShouYi;                     //投资收益		30
        public float yywShouZhi;                   //营业外收支(万元)
        public float zongLiRun;                    //利润总额(万元)
        public float jingLiRun;                    //净利润(千元)
        public float weiFenPei;                    //未分配利润(万元)
        public float zongGu;                       //总股本(万股)
        public float wxsAg;                        //无限售股合计
        public float liuTongAG;                    //流通A股(万股)
        public float bGu;                          //B股(万股)
        public float jingwaiGu;                    //境外上市股(万股)
        public float qtltg;                        //其它流通股	40
        public float xsghj;                        //限售股合计
        public float guojiaGu;                     //国家股(万股)
        public float farenGu;                      //国有法人股
        public float jnFaRenGu;                    //境内法人股(万股)
        public float jnZiRanGu;                    //境内自然人股
        public float qitaGu;                       //其他发起人股(万股)
        public float mujiGu;                       //募集法人股
        public float jingWaiGu;                    //境外法人股
        public float jwZiRanGu;                    //境外自然人股
        public float youxianGu;                    //优先股或其他	50(大智慧204字节结构)

        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 58)]
        public byte[] temp;                        //系统预留*
        public uint Time()
        {
            return Time(time);
        }
        public uint BaoGao()
        {
            return Time(baoGao);
        }
        public uint SangShi()
        {
            return Time(shangShi);
        }
        uint Time(int date)
        {
            int year = date / 10000;
            int mon = (date % 10000) / 100;
            int day = date % 100;
            DateTime t = new DateTime(year, mon, day);

            DateTime dtZone = new DateTime(1970, 1, 1, 8, 0, 0);

            uint time = (uint)t.Subtract(dtZone).TotalSeconds;

            return time;
        }
    };
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode, Pack = 1)]
    struct OEM_F10INFO        //F10资料，每一节索引50字节
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 12)]
        public string title;                       //代码
        public int from;                           //文件位置
        public int len;                            //长度
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 18)]
        public byte[] temp;                        //系统预留*
    };
}
