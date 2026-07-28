import frida, sys

def on_message(message, data):
    if message['type'] == 'send':
        print(f"[*] {message['payload']}")
    else:
        print(message)

js_code = """
var p_create = Module.findExportByName('kernel32.dll', 'CreateFileW');
if (p_create) {
    Interceptor.attach(p_create, {
        onEnter: function(args) {
            var path = args[0].readUtf16String();
            if (path && (path.indexOf('Stock') !== -1 || path.indexOf('Netzip') !== -1)) {
                console.log("[*] CreateFileW: " + path);
            }
        }
    });
}

var p_load = Module.findExportByName('kernel32.dll', 'LoadLibraryW');
if (p_load) {
    Interceptor.attach(p_load, {
        onEnter: function(args) {
            var path = args[0].readUtf16String();
            if (path && (path.indexOf('Stock') !== -1 || path.indexOf('Netzip') !== -1)) {
                console.log("[*] LoadLibraryW: " + path);
            }
        }
    });
}
console.log("[JS] Hooks installed.");
"""

pid = 55428
device = frida.get_local_device()
session = device.attach(pid)
script = session.create_script(js_code)
script.on('message', on_message)
script.load()
print("[*] Monitoring started. Press Ctrl+C to stop.")
sys.stdin.read()
