import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Universal Plain Sniffer V4 (FoxTrader Edition) loaded.");
    var base = Module.findBaseAddress("FoxTrader.exe");
    if (!base) {
        console.log("[JS] Error: Cannot find base address for FoxTrader.exe");
        return;
    }
    
    // 0x694ed logic
    var target = base.add(0x694ed); 
    Interceptor.attach(target, {
        onEnter: function(args) {
            var buf = args[0];
            var len = args[1].toInt32();
            if (len > 0) {
                var data = buf.readByteArray(len);
                console.log("[Wrapper] Sending " + len + " bytes plain text.");
                send({type: "packet", source: "wrapper", len: len}, data);
            }
        }
    });

    // Also monitor Winsock
    var sendPtr = Module.findExportByName("ws2_32.dll", "send");
    if (sendPtr) {
        Interceptor.attach(sendPtr, {
            onEnter: function(args) {
                var buf = args[1];
                var len = args[2].toInt32();
                if (len > 50) {
                    var data = buf.readByteArray(len);
                    send({type: "packet", source: "winsock", len: len}, data);
                }
            }
        });
    }

    console.log("[JS] Hooks active on FoxTrader.exe (" + base + ")");
})();
"""

def on_message(message, data):
    if message['type'] == 'send':
        payload = message['payload']
        length = payload['len']
        src = payload['source']
        filename = f"trace_{src}_{length}_{int(time.time()*1000)}.bin"
        with open(os.path.join(r"Z:\netzipapi-rust-demo", filename), 'wb') as f:
            f.write(data)
        print(f"[*] Captured {src} -> {length} bytes -> {filename}")

try:
    # Use name-based attach
    session = frida.attach("FoxTrader.exe")
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Sniffer V4 active on FoxTrader.exe. Please click 300654 K-line again.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
