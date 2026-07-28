import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        name = message['payload'].get('name', 'unknown.bin')
        path = os.path.join(r'Z:\netzipapi-rust-demo', name)
        with open(path, 'wb') as f: f.write(data)
        print(f"[*] SUCCESS: Captured {len(data)} bytes -> {path}")
    elif message['type'] == 'error':
        print(f"[JS ERROR] {message['stack']}")
    else:
        print(f"[*] {message}")

js_code = """
var attached_net = false;
var attached_stock = false;

function doHook() {
    try {
        if (!attached_net) {
            var p_wsasend = Module.findExportByName(null, "WSASend");
            if (p_wsasend) {
                Interceptor.attach(p_wsasend, {
                    onEnter: function(args) {
                        try {
                            var dwBufferCount = args[2].toInt32();
                            var lpBuffers = args[1];
                            for (var i = 0; i < dwBufferCount; i++) {
                                var len = lpBuffers.add(i * 8).readU32();
                                if (len >= 280 && len <= 300) {
                                    var buf = lpBuffers.add(i * 8 + 4).readPointer();
                                    send({name: "cipher_0118_wsasend.bin"}, buf.readByteArray(len));
                                }
                            }
                        } catch(e) {}
                    }
                });
                console.log("[JS] WSASend hooked.");
                attached_net = true;
            }
            
            var p_send = Module.findExportByName(null, "send");
            if (p_send) {
                Interceptor.attach(p_send, {
                    onEnter: function(args) {
                        try {
                            var len = args[2].toInt32();
                            if (len >= 280 && len <= 300) {
                                send({name: "cipher_0118_send.bin"}, args[1].readByteArray(len));
                            }
                        } catch(e) {}
                    }
                });
                console.log("[JS] send hooked.");
                attached_net = true;
            }
        }

        if (!attached_stock) {
            var m = Process.findModuleByName("Stock.dat");
            if (m) {
                var addr_plain = m.base.add(0x66b60);
                Interceptor.attach(addr_plain, {
                    onEnter: function(args) { this.sav = [args[0], args[1], args[2], args[3]]; },
                    onLeave: function(retval) {
                        for (var i=0; i<4; i++) {
                            try {
                                var val = this.sav[i].toInt32();
                                if (val === 280) { send({name: "plain_0118.bin"}, this.sav[i-1].readByteArray(280)); }
                                else if (val === 282) { send({name: "plain_0118.bin"}, this.sav[i-1].add(2).readByteArray(280)); }
                            } catch(e) {}
                        }
                    }
                });
                console.log("[JS] Stock.dat (0x66b60) hooked.");
                attached_stock = true;
            }
        }
    } catch(e) { }
}
setTimeout(doHook, 100);
setInterval(doHook, 2000);
"""

pids = [int(x) for x in sys.argv[1:]]
device = frida.get_local_device()
for pid in pids:
    try:
        session = device.attach(pid)
        script = session.create_script(js_code)
        script.on('message', on_message)
        script.load()
        print(f"[+] Hooked PID {pid}")
    except Exception as e:
        print(f"[!] Error PID {pid}: {e}")

print("[*] Monitoring... Press Ctrl+C to stop.")
sys.stdin.read()
