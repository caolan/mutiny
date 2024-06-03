import { connect, defaultSocketPath } from "../lib/client.ts";

if (import.meta.main) {
    const socket_path = defaultSocketPath(); 
    const client = await connect({socket_path});
    console.log(await client.localPeerId());
}
