import { defaultSocketPath } from "../client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { help } from "./help.ts";
import { MutinyClient } from "../client.ts";
import { Server } from "../server.ts";

export default async function (args: ReturnType<typeof parseArgs>) {
    if (args._.length < 2 || args.help) {
        help('serve');
    }
    const socket_path = args.s || args.socket || defaultSocketPath(); 
    const label = "" + args._[0];
    const root = "" + args._[1];

    const client = new MutinyClient({socket_path});
    const uuid = (
        await client.appInstanceUuid(label) ?? 
        await client.createAppInstance(label)
    );
    const server = new Server(client, {label, uuid}, root);
    await server.serve();
}
