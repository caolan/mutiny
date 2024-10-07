import { help, listCommands } from "./commands/help.ts";
import { parseArgs } from "@std/cli/parse-args";
import serve from "./commands/serve.ts";
import info from "./commands/info.ts";
import dial from "./commands/dial.ts";
import peers from "./commands/peers.ts";

if (import.meta.main) {
    const args = parseArgs(Deno.args);
    const command = args._.shift();
    if (!command) {
        help(args.help);
        Deno.exit(1);
    }
    switch (command) {
        case "serve": {
            serve(args);
            break;
        }
        case "info": {
            info(args);
            break;
        }
        case "dial": {
            dial(args);
            break;
        }
        case "peers": {
            peers(args);
            break;
        }
        default: {
            console.error(`Unknown command: ${command}`);
            console.error("");
            listCommands();
            Deno.exit(1);
        }
    }
}
