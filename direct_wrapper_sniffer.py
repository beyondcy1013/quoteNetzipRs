import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Direct Wrapper Sniffer loaded.");
    var base = Module.getBaseAddress("网际风.exe");
    var target = base.add(0x694ed); // 0x4694ed - 0x400000 assuming base 0x400000

    Interceptor.attach(target, {
        onEnter: function(args) {
            // This function is the wrapper identified at 0x4694ed
            // Based on previous traces: arg[0] is buffer, arg[1] is len
            var buf = args[0];
            var len = args[1].toInt32();
            if (len > 0) {
                var data = buf.readByteArray(len);
                console.log("[Wrapper] Sending " + len + " bytes plain text.");
                send({type: "packet", len: len}, data);
            }
        }
    });
    console.log("[JS] Hooked 网际风.exe + 0x694ed (" + target + ")");
})();
"""

def on_message(message, data):
    if message['type'] == 'send':
        payload = message['payload']
        length = payload['len']
        filename = f"wrapper_{length}_{int(time.time()*1000)}.bin"
        with open(os.path.join(r"Z:\netzipapi-rust-demo", filename), 'wb') as f:
            f.write(data)
        print(f"[*] Captured {length} byte plain text -> {filename}")

PID = 64224
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Ready. Please click 300654 K-line now.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
