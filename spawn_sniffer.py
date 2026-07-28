import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        p = message['payload']
        if p.get('type') == 'log':
            msg = f"[*] {p.get('func', 'LOG')} - Len: {p['len']} HEX: {p['hex']}"
            if p.get('bt'):
                msg += f"\n    BT:\n{p['bt']}"
            print(msg)
        elif p.get('type') == 'dump':
            name = p.get('name', f"spawn_dump_{p['len']}_{int(time.time())}.bin")
            with open(os.path.join(r'Z:\netzipapi-rust-demo', name), 'wb') as f:
                f.write(data)
            print(f"[*] Dumped {len(data)} bytes -> {name}")
    elif message['type'] == 'error':
        print(f"[JS ERR] {message['stack']}")
    else:
        print(message)

js_code = """
(function() {
    var attached_net = false;
    
    function getExp(mod, name) {
        var m = Process.findModuleByName(mod);
        if (!m) return null;
        return m.getExportByName(name);
    }

    function hook_io() {
        var p_create = getExp("kernel32.dll", "CreateFileW");
        if (p_create) {
            Interceptor.attach(p_create, {
                onEnter: function(args) {
                    try {
                        var path = args[0].readUtf16String();
                        if (path && (path.indexOf("Stock") !== -1 || path.indexOf("Netzip") !== -1)) {
                            send({type: "log", len: 0, hex: "", func: "CreateFileW", bt: path});
                        }
                    } catch(e) {}
                }
            });
        }
    }

    function hook_net() {
        if (attached_net) return;
        var ws2 = Process.findModuleByName("ws2_32.dll");
        if (!ws2) return;
        
        var p_wsasend = ws2.findExportByName("WSASend");
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
                                try { hex = ptr.readByteArray(Math.min(len, 16)).slice(0); } catch(e) {}
                                var bt = "";
                                if (len >= 280) {
                                    try {
                                        bt = Thread.backtrace(this.context, 1).map(function(addr){ return DebugSymbol.fromAddress(addr).toString(); }).join("\\n");
                                    } catch(e) { bt = "BT failed: " + e; }
                                }
                                send({type: "log", len: len, hex: hex, func: "WSASend", bt: bt});
                                if (len >= 200) {
                                    send({type: "dump", len: len}, ptr.readByteArray(len));
                                }
                            }
                        }
                    } catch(e) {}
                }
            });
        }

        var p_send = ws2.findExportByName("send");
        if (p_send) {
            Interceptor.attach(p_send, {
                onEnter: function(args) {
                    try {
                        var len = args[2].toInt32();
                        var ptr = args[1];
                        if (len > 0) {
                            var hex = "";
                            try { hex = ptr.readByteArray(Math.min(len, 16)).slice(0); } catch(e) {}
                            var bt = "";
                            if (len >= 280) {
                                try {
                                    bt = Thread.backtrace(this.context, 1).map(function(addr){ return DebugSymbol.fromAddress(addr).toString(); }).join("\\n");
                                } catch(e) { bt = "BT failed: " + e; }
                            }
                            send({type: "log", len: len, hex: hex, func: "send", bt: bt});
                            if (len >= 200) {
                                send({type: "dump", len: len}, ptr.readByteArray(len));
                            }
                        }
                    } catch(e) {}
                }
            });
        }
        
        attached_net = true;
        console.log("[JS] Network Hooks set.");
    }

    hook_io();
    hook_net();
    setInterval(hook_net, 1000);
})();
"""

device = frida.get_local_device()
pid = device.spawn([r"Z:\netzipapi-rust-demo\netzip_api_bin\NetzipAPI\StockC#\网际风.exe"])
session = device.attach(pid)
script = session.create_script(js_code)
script.on('message', on_message)
script.load()
device.resume(pid)
print(f"[*] Spawned and Resumed PID {pid}. Let's go!")
sys.stdin.read()
