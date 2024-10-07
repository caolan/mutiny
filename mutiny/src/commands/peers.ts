import { defaultSocketPath } from "../client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { help } from "./help.ts";
import { MutinyClient } from "../client.ts";

export default async function (args: ReturnType<typeof parseArgs>) {
    if (args.help) {
        help('peers');
    }
    const socket_path = args.s || args.socket || defaultSocketPath(); 

    const client = new MutinyClient({socket_path});
    const peers = await client.peers();
    for (const peer of peers) {
        console.log(peer);
    }
}
