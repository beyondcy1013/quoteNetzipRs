import frida, sys

def on_message(message, data):
    print(message)

js_code = """
Process.enumerateModules().forEach(function(m) {
    console.log(m.name + " @ " + m.base + " (" + m.path + ")");
});
"""

PID = 60112
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    session.detach()
except Exception as e:
    print(f"Error: {e}")
