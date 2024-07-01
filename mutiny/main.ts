import { connect, defaultSocketPath } from "../lib/client.ts";

if (import.meta.main) {
    const socket_path = Deno.args[0] || defaultSocketPath(); 
    const client = await connect({socket_path});
    console.log(await client.localPeerId());
    console.log(await client.peers());
}
