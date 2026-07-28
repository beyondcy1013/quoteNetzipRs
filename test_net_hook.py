import frida, sys, os

def on_message(message, data):
    if message['type'] == 'send':
        print(f"[*] Payload: {message['payload']}")
        if data:
            name = message['payload'].get('name', 'dump.bin')
            with open(name, 'wb') as f: f.write(data)
            print(f"[*] Dumped {len(data)} bytes to {name}")
    else:
        print(message)

js = """
var ws2 = Module.findExportByName("ws2_32.dll", "WSASend");
if (ws2) {
    console.log("[JS] Hooking WSASend at " + ws2);
    Interceptor.attach(ws2, {
        onEnter: function(args) {
            var cnt = args[2].toInt32();
            var bufs = args[1];
            for (var i=0; i<cnt; i++) {
                var len = bufs.add(i*8).readU32();
                console.log("[JS] WSASend length: " + len);
                if (len >= 200 && len <= 400) {
                    var ptr = bufs.add(i*8+4).readPointer();
                    send({name: "cipher_" + len + ".bin"}, ptr.readByteArray(len));
                }
            }
        }
    });
}
var snd = Module.findExportByName("ws2_32.dll", "send");
if (snd) {
    console.log("[JS] Hooking send at " + snd);
    Interceptor.attach(snd, {
        onEnter: function(args) {
            var len = args[2].toInt32();
            console.log("[JS] send length: " + len);
            if (len >= 200 && len <= 400) {
                send({name: "cipher_send_" + len + ".bin"}, args[1].readByteArray(len));
            }
        }
    });
}
"""

if len(sys.argv) < 2:
    print("Usage: python test_net_hook.py <pid>")
    sys.exit(1)

pid = int(sys.argv[1])
session = frida.attach(pid)
script = session.create_script(js)
script.on('message', on_message)
script.load()
print(f"[*] Hooked PID {pid}. Press Ctrl+C to stop.")
sys.stdin.read()
