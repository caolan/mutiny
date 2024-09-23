import { defaultSocketPath } from "../client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { help } from "./help.ts";
import { MutinyClient } from "../client.ts";

export default async function (args: ReturnType<typeof parseArgs>) {
    if (args.help) {
        help('info');
    }
    const socket_path = args.s || args.socket || defaultSocketPath(); 

    const client = new MutinyClient({socket_path});
    const peer_id = await client.localPeerId();
    console.log(`Mutinyd socket: ${socket_path}`);
    console.log(`Local peer ID: ${peer_id}`);
}
