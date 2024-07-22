import { connect, defaultSocketPath } from "../lib/client.ts";

if (import.meta.main) {
    const socket_path = Deno.args[0] || defaultSocketPath(); 
    const client = await connect({socket_path});
    const [peer_id, peers] = await Promise.all([
        client.peers(),
        client.localPeerId(),
    ]);
    console.log(peer_id);
    console.log(peers);
}
