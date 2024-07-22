import { connect, defaultSocketPath, MutinyClient, Message } from "../../lib/client.ts";
import { parseArgs } from "@std/cli/parse-args";
import { serveDir } from "@std/http";

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
            let stop = false;
            const client = this.client;
            const body = new ReadableStream({
                start(controller) {
                    (async () => {
                        const encoder = new TextEncoder();
                        for await (const event of client.peerEvents()) {
                            if (stop) break;
                            controller.enqueue(
                                encoder.encode(
                                    `event: ${event.type}\r\ndata: ${event.peer_id}\r\n\r\n`
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
        } else if (request.method === 'POST' && pathname === '/_api/v1/message_send') {
            const body = await request.json();
            const message = new TextEncoder().encode(body.message);
            return new Response(JSON.stringify(await this.client.messageSend(
                body.peer,
                body.app_uuid,
                this.app.uuid,
                message,
            )));
        } else if (pathname === '/_api/v1/announcements') {
            if (request.method === 'POST') {
                const body = await request.json();
                await this.client.announce(
                    body.peer,
                    this.app.uuid,
                    body.data,
                );
                return new Response(JSON.stringify({success: true}));
            } else {
                return new Response(JSON.stringify(await this.client.announcements()));
            }
        } else if (pathname === '/_api/v1/message_read') {
            const m = await this.client.messageRead(this.app.uuid) as Message;
            return new Response(JSON.stringify(m && {
                peer: m.peer,
                uuid: m.uuid,
                message: new TextDecoder().decode(m.message),
            }));
        } else if (request.method === 'POST' && pathname === '/_api/v1/message_next') {
            await this.client.messageNext(this.app.uuid);
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
