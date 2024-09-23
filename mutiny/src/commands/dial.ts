import { defaultSocketPath } from "../client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { help } from "./help.ts";
import { MutinyClient } from "../client.ts";

export default async function (args: ReturnType<typeof parseArgs>) {
    if (args._.length < 1 || args.help) {
        help('dial');
    }
    const socket_path = args.s || args.socket || defaultSocketPath(); 
    const address = "" + args._[0];

    const client = new MutinyClient({socket_path});
    await client.dialAddress(address);
}
