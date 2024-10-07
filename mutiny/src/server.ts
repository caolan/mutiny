import { MutinyClient } from "./client.ts";
import { serveDir } from "@std/http";
import eventStream from "./eventstream.ts";

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
        try {
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
            } else if (request.method === 'POST' && pathname === '/_api/v1/dial') {
                const body = await request.json();
                await this.client.dialAddress(body.address);
                return new Response(JSON.stringify({success: true}));
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
        } catch (err) {
            console.log(`Internal Server Error: ${err.message}`);
            return new Response('Internal Server Error', {status: 500});
        }
    }

    handleRequest(request: Request): Promise<Response> {
        return this.isAPIRequest(request) ? 
            this.serveAPI(request) : 
            serveDir(request, {fsRoot: this.root})
    }

    async serve(): Promise<Deno.HttpServer<Deno.NetAddr>> {
        const onListen = async (addr: Deno.NetAddr) => {
            console.log("Application:");
            console.log(`  uuid: ${this.app.uuid}`);
            console.log(`  label: ${this.app.label}`);
            console.log("");
            console.log(`Serving ${this.root}:`);
            console.log(`  http://${addr.hostname}:${addr.port}/`);
            // Update last used port so we can attempt to use it again
            // on restart.
            await this.client.setLastPort(this.app.uuid, addr.port);
        };
        const hostname = '127.0.0.1';
        const port = await this.client.getLastPort(this.app.uuid) ?? 0;
        try {
            return Deno.serve({
                onListen,
                hostname,
                port,
            }, this.handleRequest.bind(this));
        } catch (err) {
            if (err.code === 'EADDRINUSE') {
                // Address already in use, get a new randomly assigned port
                return Deno.serve({
                    onListen,
                    hostname,
                    port: 0,
                }, this.handleRequest.bind(this));
            } else {
                throw err;
            }
        }
    }
}
