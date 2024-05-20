import { serveDir } from "https://deno.land/std@0.224.0/http/file_server.ts";
import { connect, defaultSocketPath, FEClient } from "../lib/client.ts";

class Server {
    constructor (
        private client: FEClient,
        private root: string,
    ) {}

    isAPIRequest(request: Request): boolean {
        const pathname = new URL(request.url).pathname;
        return new RegExp('^/_api(?:/.*)?$').test(pathname);
    }

    async serveAPI(request: Request) {
        const url = new URL(request.url);
        const pathname = url.pathname;
        if (pathname === '/_api/ping') {
            return new Response(await this.client.ping());
        } else {
            return new Response(`API response for ${pathname}`);
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
                console.log(`Serving ${this.root}`);
                console.log(`  from http://${addr.hostname}:${addr.port}/`);
            },
        }, this.handleRequest.bind(this));
    }
}

if (import.meta.main) {
    const socket_path = defaultSocketPath(); 
    const root = Deno.args[0] || '.';

    const client = await connect({socket_path});
    const server = new Server(client, root);

    server.serve();
}
