import frida, sys, time, os

js_code = """
function extract() {
    var addr = ptr("0x1eaf2580").sub(32); // Adjust to get head
    var buf = addr.readByteArray(280);
    send({type: "dump", len: 280}, buf);
}
setTimeout(extract, 500);
"""

def on_message(message, data):
    if message['type'] == 'send':
        p = message['payload']
        if p.get('type') == 'dump':
            with open(r'Z:\netzipapi-rust-demo\plain_0118.bin', 'wb') as f:
                f.write(data)
            print("[*] Successfully extracted plain_0118.bin (280 bytes)")

PID = 60112
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    time.sleep(2)
    session.detach()
except Exception as e:
    print(f"Error: {e}")
