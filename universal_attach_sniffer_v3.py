import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Universal Attach Sniffer V3 (THE FINAL ONE) loaded.");

    function dump(prefix, buf, len) {
        if (len <= 0) return;
        var data = buf.readByteArray(len);
        var uint8 = new Uint8Array(data);
        var s = "";
        for (var i = 0; i < Math.min(len, 200); i++) {
            if (uint8[i] >= 32 && uint8[i] <= 126) s += String.fromCharCode(uint8[i]);
            else s += ".";
        }
        
        var hit = (s.indexOf("30065") !== -1 || s.indexOf("168") !== -1);
        if (hit || len > 50) {
            console.log("\\n[" + prefix + "] len=" + len + (hit ? " [HIT!]" : ""));
            send({type: "packet", prefix: prefix, len: len}, data);
        }
    }

    // Hook send
    var p_send = Module.findExportByName("ws2_32.dll", "send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                dump("send", args[1], args[2].toInt32());
            }
        });
    }

    // Hook WSASend
    var p_WSASend = Module.findExportByName("ws2_32.dll", "WSASend");
    if (p_WSASend) {
        Interceptor.attach(p_WSASend, {
            onEnter: function(args) {
                var count = args[2].toInt32();
                var buffers = args[1];
                for (var i = 0; i < count; i++) {
                    var len = buffers.add(i * 8).readU32(); // 32-bit: len is first 4 bytes
                    var ptr = buffers.add(i * 8 + 4).readPointer();
                    dump("WSASend", ptr, len);
                }
            }
        });
    }

    // Hook recv (to catch response)
    var p_recv = Module.findExportByName("ws2_32.dll", "recv");
    if (p_recv) {
        Interceptor.attach(p_recv, {
            onLeave: function(retval) {
                var len = retval.toInt32();
                if (len > 0) dump("recv", this.args[1], len);
            }
        });
    }
    
    // Hook WSARecv
    var p_WSARecv = Module.findExportByName("ws2_32.dll", "WSARecv");
    if (p_WSARecv) {
        Interceptor.attach(p_WSARecv, {
            onLeave: function(retval) {
                var count = this.args[2].toInt32();
                var buffers = this.args[1];
                for (var i = 0; i < count; i++) {
                    var len = buffers.add(i * 8).readU32();
                    var ptr = buffers.add(i * 8 + 4).readPointer();
                    dump("WSARecv", ptr, len);
                }
            }
        });
    }

    console.log("[JS] Hooks active: send, WSASend, recv, WSARecv.");
})();
"""

def on_message(message, data):
    if message['type'] == 'send':
        payload = message['payload']
        if payload['type'] == 'packet':
            prefix = payload['prefix']
            length = payload['len']
            timestamp = int(time.time() * 1000)
            filename = f"trace_{prefix}_{length}_{timestamp}.bin"
            filepath = os.path.join(r"Z:\netzipapi-rust-demo", filename)
            with open(filepath, 'wb') as f:
                f.write(data)
            print(f"[*] Captured {prefix} {length} bytes -> {filename}")

PID = 64224
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Sniffer V3 active on PID {PID}. Please re-request 300654/300655.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
