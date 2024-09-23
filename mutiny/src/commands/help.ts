export function listCommands() {
    console.error("Commands:");
    console.error("  serve    Serve an application");
    console.error("  info     Show info about mutinyd");
    console.error("  dial     Connects to a known multi-address");
}

export function help(command?: string) {
    switch (command) {
        case "serve": {
            console.error("Usage: mutiny serve [OPTIONS] LABEL PATH");
            console.error("");
            console.error("Arguments:");
            console.error("  LABEL  Label of app instance");
            console.error("  PATH   Path to static assets");
            console.error("");
            console.error("Options:");
            console.error("  -s, --socket <SOCKET>  Unix socket to bind to");
            console.error("  --help                 Show this message");
            break;
        }
        case "info": {
            console.error("Usage: mutiny info [OPTIONS]");
            console.error("");
            console.error("Options:");
            console.error("  -s, --socket <SOCKET>  Unix socket to bind to");
            console.error("  --help                 Show this message");
            break;
        }
        case "dial": {
            console.error("Usage: mutiny dial [OPTIONS] ADDRESS");
            console.error("");
            console.error("Arguments:");
            console.error("  ADDRESS  libp2p multi-address to connect to");
            console.error("");
            console.error("Options:");
            console.error("  -s, --socket <SOCKET>  Unix socket to bind to");
            console.error("  --help                 Show this message");
            break;
        }
        default: {
            console.error("Usage: mutiny COMMAND");
            console.error("");
            listCommands();
        }
    }
    Deno.exit(1);
}
