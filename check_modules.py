import frida, sys

def main():
    pid = int(sys.argv[1])
    session = frida.attach(pid)
    modules = session.enumerate_modules()
    for m in modules:
        print(f"{m.name:30} {m.base} {m.path}")
    session.detach()

if __name__ == "__main__":
    main()
