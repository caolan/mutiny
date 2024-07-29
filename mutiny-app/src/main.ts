import { connect, defaultSocketPath, MutinyClient } from "../../lib/client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { serveDir } from "@std/http";

function eventStream<T>(iter: AsyncIterableIterator<T>, map: (event: T) => [string, string]) {
    let stop = false;
    const body = new ReadableStream({
        start(controller) {
            (async () => {
                const encoder = new TextEncoder();
                for await (const event of iter) {
                    if (stop) break;
                    const [name, data] = map(event);
                    controller.enqueue(
                        encoder.encode(
                            `event: ${name}\r\ndata: ${data}\r\n\r\n`
                        )
                    );
                }
            })();
        },
        cancel() {
            stop = true;
        }
    });
    return new Response(body, {
        headers: {
            'Content-Type': 'text/event-stream',
        }
    });
}

export class Server {
    constructor (
        private client: MutinyClient,
        private app: {
            label: string,
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
        if (pathname === '/_api/v1/application') {
            return new Response(JSON.stringify(this.app));
        } else if (pathname === '/_api/v1/local_peer_id') {
            return new Response(await this.client.localPeerId());
        } else if (pathname === '/_api/v1/peers') {
            return new Response(JSON.stringify(await this.client.peers()));
        } else if (pathname === '/_api/v1/peers/events') {
            return eventStream(this.client.peerEvents(), event => {
                return [event.type, event.peer_id];
            });
        } else if (request.method === 'POST' && pathname === '/_api/v1/announcements/outbox') {
            const body = await request.json();
            await this.client.announce(
                body.peer,
                this.app.uuid,
                body.data,
            );
            return new Response(JSON.stringify({success: true}));
        } else if (pathname === '/_api/v1/announcements/inbox') {
            return new Response(JSON.stringify(await this.client.announcements()));
        } else if (pathname === '/_api/v1/announcements/inbox/events') {
            return eventStream(this.client.announceEvents(), event => {
                return [event.type, JSON.stringify({
                    peer: event.peer, 
                    app_uuid: event.app_uuid,
                    data: event.data,
                })];
            });
        } else if (request.method === 'POST' && pathname === '/_api/v1/messages/outbox') {
            const body = await request.json();
            const message = new TextEncoder().encode(body.message);
            return new Response(JSON.stringify(await this.client.sendMessage(
                body.peer,
                body.app_uuid,
                this.app.uuid,
                message,
            )));
        } else if (request.method === 'GET' && pathname === '/_api/v1/messages/inbox') {
            const messages = await this.client.inboxMessages(this.app.uuid);
            return new Response(JSON.stringify(messages.map(m => ({
                id: m.id,
                peer: m.peer,
                uuid: m.uuid,
                message: new TextDecoder().decode(m.message),
            }))));
        } else if (request.method === 'DELETE' && pathname === '/_api/v1/messages/inbox') {
            const body = await request.json();
            await this.client.deleteInboxMessage(this.app.uuid, body.message_id);
            return new Response(JSON.stringify({success: true}));
        } else if (pathname === '/_api/v1/messages/inbox/events') {
            return eventStream(this.client.inboxEvents(this.app.uuid), event => {
                return [event.type, JSON.stringify({
                    id: event.id,
                    peer: event.peer, 
                    uuid: event.uuid,
                    message: new TextDecoder().decode(event.message),
                })];
            });
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
                console.log("Application:");
                console.log(`  uuid: ${this.app.uuid}`);
                console.log(`  label: ${this.app.label}`); 
                console.log("");
                console.log(`Serving ${this.root}:`);
                console.log(`  http://${addr.hostname}:${addr.port}/`);
            },
        }, this.handleRequest.bind(this));
    }
}

if (import.meta.main) {
    if (Deno.args.length < 2) {
        console.error("Usage: mutiny-app [OPTIONS] LABEL PATH");
        console.error("");
        console.error("Options:");
        console.error("  -s, --socket <SOCKET>  Unix socket to bind to");
        Deno.exit(1);
    }
    const args = parseArgs(Deno.args);
    const socket_path = args.s || args.socket || defaultSocketPath(); 
    const label = "" + args._[0];
    const root = "" + args._[1];

    const client = await connect({socket_path});
    const uuid = (
        await client.appInstanceUuid(label) ?? 
        await client.createAppInstance(label)
    );

    const server = new Server(client, {label, uuid}, root);
    server.serve();
}
