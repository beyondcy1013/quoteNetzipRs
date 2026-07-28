import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Universal Attach Sniffer V2 (PERMISSIVE) loaded.");

    var p_send = Module.findExportByName("ws2_32.dll", "send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                var len = args[2].toInt32();
                // PERMISSIVE: Catch all packets to find requests
                var ptr = args[1];
                var data = ptr.readByteArray(len);
                
                // Optional: Console log for 300654 string
                var uint8 = new Uint8Array(data);
                var s = "";
                for(var i=0; i<Math.min(len, 100); i++) s += String.fromCharCode(uint8[i]);
                
                if (s.indexOf("300654") !== -1 || len > 100) {
                     console.log("\\n[send] len=" + len + (s.indexOf("300654") !== -1 ? " (300654 FOUND!)" : ""));
                     send({type: "packet", len: len}, data);
                }
            }
        });
        console.log("[JS] send hooked.");
    }
})();
"""

def on_message(message, data):
    if message['type'] == 'send':
        payload = message['payload']
        if payload['type'] == 'packet':
            length = payload['len']
            timestamp = int(time.time() * 1000)
            # Use specific name for easy identification
            filename = f"trace_{length}_{timestamp}.bin"
            filepath = os.path.join(r"Z:\netzipapi-rust-demo", filename)
            with open(filepath, 'wb') as f:
                f.write(data)
            print(f"[*] Captured {length} bytes -> {filename}")

PID = 64224
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Sniffer V2 active on PID {PID}. Please re-request K-line for 300654.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
