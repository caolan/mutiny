import { connect, defaultSocketPath, MutinyClient, Message } from "../../lib/client.ts";
import { readManifest } from "../../lib/manifest.ts";
import { serveDir } from "@std/http";
import { join } from "@std/path";

export class Server {
    constructor (
        private client: MutinyClient,
        private instance: {
            name: string,
            uuid: string,
        },
        private root: string,
    ) {}

    isAPIRequest(request: Request): boolean {
        const pathname = new URL(request.url).pathname;
        return new RegExp('^/_api(?:/.*)?$').test(pathname);
    }

    async serveAPI(request: Request) {
        const url = new URL(request.url);
        const pathname = url.pathname;
        if (pathname === '/_api/v1/application_instance') {
            return new Response(JSON.stringify(this.instance));
        } else if (pathname === '/_api/v1/local_peer_id') {
            return new Response(await this.client.localPeerId());
        } else if (pathname === '/_api/v1/peers') {
            return new Response(JSON.stringify(await this.client.peers()));
        } else if (request.method === 'POST' && pathname === '/_api/v1/message_invite') {
            const data = await request.json();
            await this.client.messageInvite(
                data.peer,
                this.instance.uuid,
            );
            return new Response(JSON.stringify({success: true}));
        } else if (request.method === 'POST' && pathname === '/_api/v1/message_send') {
            const data = await request.json();
            const message = new TextEncoder().encode(data.message);
            return new Response(JSON.stringify(await this.client.messageSend(
                data.peer,
                data.app_instance_uuid,
                this.instance.uuid,
                message,
            )));
        } else if (pathname === '/_api/v1/message_invites') {
            return new Response(JSON.stringify(await this.client.messageInvites()));
        } else if (pathname === '/_api/v1/message_read') {
            const m = await this.client.messageRead(this.instance.uuid) as Message;
            return new Response(JSON.stringify(m && {
                peer: m.peer,
                uuid: m.uuid,
                message: new TextDecoder().decode(m.message),
            }));
        } else if (request.method === 'POST' && pathname === '/_api/v1/message_next') {
            await this.client.messageNext(this.instance.uuid);
            return new Response(JSON.stringify({success: true}));
        } else {
            return new Response('Not found', {status: 404});
        }
    }

    handleRequest(request: Request): Promise<Response> {
        return this.isAPIRequest(request) ? 
            this.serveAPI(request) : 
            serveDir(request, {fsRoot: this.root})
    }

    serve(): Deno.HttpServer<Deno.NetAddr> {
        return Deno.serve({
            hostname: '127.0.0.1',
            port: 0,
            onListen: addr => {
                console.log("Application instance:");
                console.log(`  uuid: ${this.instance.uuid}`);
                console.log(`  name: ${this.instance.name}`); 
                console.log("");
                console.log(`Serving ${this.root}:`);
                console.log(`  http://${addr.hostname}:${addr.port}/`);
            },
        }, this.handleRequest.bind(this));
    }
}

if (import.meta.main) {
    if (Deno.args.length < 1) {
        console.error("Usage: mutiny-app INSTANCE_NAME [PATH]");
        Deno.exit(1);
    }
    const socket_path = defaultSocketPath(); 
    const name = Deno.args[0];
    const root = Deno.args[1] || '.';
    const manifest = await readManifest(join(root, "mutiny.json"));

    const client = await connect({socket_path});
    const uuid = (
        await client.appInstanceUuid(name) ?? 
        await client.createAppInstance(name, manifest)
    );

    const server = new Server(client, {name, uuid}, root);
    server.serve();
}
