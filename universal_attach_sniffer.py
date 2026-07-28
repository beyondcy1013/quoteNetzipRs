import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Universal Attach Sniffer loaded.");

    // 1. Hook send to catch all encrypted traffic
    var p_send = Module.findExportByName("ws2_32.dll", "send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                var len = args[2].toInt32();
                if (len > 200) {
                    var ptr = args[1];
                    var data = ptr.readByteArray(len);
                    console.log("\\n[send] len=" + len);
                    
                    var bt = Thread.backtrace(this.context, 1).map(function(addr) {
                        try { return DebugSymbol.fromAddress(addr).toString(); } catch(e) { return addr.toString(); }
                    }).join("\\n");
                    
                    send({type: "packet", len: len, bt: bt}, data);
                }
            }
        });
        console.log("[JS] send hooked.");
    }

    // 2. Hook Ask/Start to catch plain text inputs
    function scanAsk() {
        var pattern = "55 8b ec 6a fe 68 f8 42 12 10"; 
        Process.enumerateRanges('r-x').forEach(function (range) {
            try {
                var ms = Memory.scanSync(range.base, range.size, pattern);
                ms.forEach(function (m) {
                    console.log("[JS] Found Ask Entry at " + m.address);
                    Interceptor.attach(m.address, {
                        onEnter: function(args) {
                            var captured = [];
                            for(var i=0; i<8; i++) {
                                try {
                                    var arg = this.context.esp.add(4 + i*4).readPointer();
                                    var s = arg.readAnsiString();
                                    if (s && s.length > 0) captured.push("arg[" + i + "]: " + s);
                                } catch(e) {}
                            }
                            if (captured.length > 0) {
                                console.log("\\n[Ask Call] Args Found:");
                                captured.forEach(function(l) { console.log("  " + l); });
                            }
                        }
                    });
                });
            } catch (e) {}
        });
    }
    setTimeout(scanAsk, 1000);
})();
"""

def on_message(message, data):
    if message['type'] == 'send':
        payload = message['payload']
        if payload['type'] == 'packet':
            length = payload['len']
            timestamp = int(time.time() * 1000)
            filename = f"capture_{length}_{timestamp}.bin"
            filepath = os.path.join(r"Z:\netzipapi-rust-demo", filename)
            with open(filepath, 'wb') as f:
                f.write(data)
            print(f"[*] Captured {length} byte packet -> {filename}")
        else:
            print(payload)

PID = 64224
try:
    print(f"[*] Attaching to PID {PID}...")
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Sniffer active on PID {PID}. Please perform UI operations.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
except KeyboardInterrupt:
    print("[*] Stopped.")
