import { help, listCommands } from "./commands/help.ts";
import { parseArgs } from "@std/cli/parse-args";
import serve from "./commands/serve.ts";
import info from "./commands/info.ts";

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
        default: {
            console.error(`Unknown command: ${command}`);
            console.error("");
            listCommands();
            Deno.exit(1);
        }
    }
}
