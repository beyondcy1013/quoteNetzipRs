import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        p = message['payload']
        if p.get('type') == 'log':
            print(f"[*] {p.get('func', 'WSASend')} - Len: {p['len']} HEX: {p['hex']}")
        elif p.get('type') == 'dump':
            name = p.get('name', f"dump_{p['len']}_{int(time.time())}.bin")
            with open(os.path.join(r'Z:\netzipapi-rust-demo', name), 'wb') as f:
                f.write(data)
            print(f"[*] Dumped {len(data)} bytes -> {name}")
    elif message['type'] == 'error':
        print(f"[JS ERR] {message['stack']}")
    else:
        print(message)

js_code = """
(function() {
    console.log("[JS] Process modules: " + JSON.stringify(Process.enumerateModules().map(m => m.name)));

    function hook() {
        var m_ws2 = Process.findModuleByName("ws2_32.dll");
        if (!m_ws2) return;

        var p_wsasend = m_ws2.findExportByName("WSASend");
        if (p_wsasend) {
            Interceptor.attach(p_wsasend, {
                onEnter: function(args) {
                    try {
                        var cnt = args[2].toInt32();
                        var bufs = args[1];
                        for (var i = 0; i < cnt; i++) {
                            var len = bufs.add(i * 8).readU32();
                            var ptr = bufs.add(i * 8 + 4).readPointer();
                            if (len > 0) {
                                var hex = "";
                                try { hex = ptr.readByteArray(Math.min(len, 32)).slice(0); } catch(e) {}
                                var bt = Thread.backtrace(this.context, Backtrace.ACCURATE).map(DebugSymbol.fromAddress).join("\\n");
                                send({type: "log", len: len, hex: hex, func: "WSASend", bt: bt});
                                if (len >= 200) {
                                    send({type: "dump", len: len}, ptr.readByteArray(len));
                                }
                            }
                        }
                    } catch(e) {}
                }
            });
            console.log("[JS] WSASend hooked.");
        }

        var p_send = m_ws2.findExportByName("send");
        if (p_send) {
            Interceptor.attach(p_send, {
                onEnter: function(args) {
                    try {
                        var len = args[2].toInt32();
                        var ptr = args[1];
                        if (len > 0) {
                            var hex = "";
                            try { hex = ptr.readByteArray(Math.min(len, 32)).slice(0); } catch(e) {}
                            var bt = Thread.backtrace(this.context, Backtrace.ACCURATE).map(DebugSymbol.fromAddress).join("\\n");
                            send({type: "log", len: len, hex: hex, func: "send", bt: bt});
                            if (len >= 200) {
                                send({type: "dump", len: len}, ptr.readByteArray(len));
                            }
                        }
                    } catch(e) {}
                }
            });
            console.log("[JS] send hooked.");
        }

        var m_stock = Process.findModuleByName("Stock.dat");
        if (m_stock) {
            console.log("[JS] Stock.dat found at " + m_stock.base + ", path: " + m_stock.path);
            var addr_plain = m_stock.base.add(0x66b60);
            Interceptor.attach(addr_plain, {
                onEnter: function(args) { this.sav = [args[0], args[1], args[2], args[3]]; },
                onLeave: function(retval) {
                    for (var i=0; i<4; i++) {
                        try {
                            var val = this.sav[i].toInt32();
                            if (val === 280) {
                                console.log("[JS] Found 280 Plain at index " + i);
                                send({type: "dump", len: 280, name: "plain_0118.bin"}, this.sav[i-1].readByteArray(280));
                            } else if (val === 282) {
                                console.log("[JS] Found 282 Plain at index " + i);
                                send({type: "dump", len: 280, name: "plain_0118.bin"}, this.sav[i-1].add(2).readByteArray(280));
                            }
                        } catch(e) {}
                    }
                }
            });
            console.log("[JS] Stock.dat 0x66b60 hooked.");
        }
    }

    hook();
})();
"""

if len(sys.argv) < 2:
    print("Usage: python sniffer.py <pid>")
    sys.exit(1)

pid = int(sys.argv[1])
device = frida.get_local_device()
try:
    session = device.attach(pid)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Sniffing PID {pid}. Press Ctrl+C to stop.")
    sys.stdin.read()
except KeyboardInterrupt:
    print("[*] Interrupted.")
except Exception as e:
    print(f"[!] Error: {e}")
