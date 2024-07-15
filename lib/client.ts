// A client library to talk to the mutiny daemon via unix socket

import { resolve } from "@std/path";
import { writeAll } from "@std/io";
import * as msgpack from "@std/msgpack";
import { readFullBuffer } from "./streams.ts";
import { assert } from "./assert.ts";

function ifDefined<T, R>(value: undefined | T, f: (value: T) => R): R | undefined {
    return (value === undefined) ? undefined : f(value);
}

function userRuntimePath(): string | undefined {
    if (Deno.build.os === "darwin") {
        // Just pick something sensible
        return ifDefined(
            Deno.env.get("HOME"), 
            home => resolve(home, "Library/Caches/TemporaryItems")
        );
    }
    // Assume following freedesktop.org specification (Linux etc)
    return Deno.env.get("XDG_RUNTIME_DIR");
}

function appRuntimePath(): string | undefined {
    return ifDefined(userRuntimePath(), dir => resolve(dir, "mutiny"));
}

export function defaultSocketPath(): string | undefined {
    return ifDefined(appRuntimePath(), dir => resolve(dir, "mutinyd.socket"));
}

type ConnectOptions = {
    socket_path?: string,
};

export async function connect({socket_path}: ConnectOptions): Promise<MutinyClient> {
    const path = socket_path ?? defaultSocketPath(); 
    if (!path) {
        throw new Error("Could not determine mutinyd.socket path");
    }
    console.log(`Connecting to ${path}`);
    const conn = await Deno.connect({
        transport: 'unix',
        path,
    });
    return new MutinyClient(conn);
}

export type Message = {
    peer: string,
    uuid: string,
    message: Uint8Array,
};

export type MessageInvite = {
    peer: string, 
    app_uuid: string,
};

type MutinyRequest = {type: "LocalPeerId"}
    | {type: "Peers"}
    | {type: "Invites"}
    | {type: "AppInstanceUuid", label: string}
    | {type: "CreateAppInstance", label: string}
    | {type: "MessageInvite", peer: string, app_uuid: string}
    | {type: "MessageInvites"}
    | {
        type: "MessageSend", 
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array,
    }
    | {type: "MessageRead", app_uuid: string}
    | {type: "MessageNext", app_uuid: string}
    ;

type MutinyResponse = {type: "Success"} 
    | {type: "Error", message: string}
    | {type: "LocalPeerId", peer_id: string}
    | {type: "Peers", peers: string[]}
    | {type: "AppInstanceUuid", uuid: string | null}
    | {type: "CreateAppInstance", uuid: string}
    | {type: "Message", message: null | Message}
    | {type: "MessageInvites",  invites: MessageInvite[]}
    ;

export class MutinyClient {
    private processing = false;
    private queue: {
        request: MutinyRequest, 
        resolve: (response: MutinyResponse) => void,
        reject: (err: Error) => void,
    }[] = [];

    constructor(
        private conn: Deno.UnixConn,
    ) {}

    private async processQueue() {
        while (this.queue.length > 0) {
            const items = this.queue;
            this.queue = [];
            for (const {request, resolve, reject} of items) {
                // reused for both request and response
                let length_buf = new ArrayBuffer(4);

                // send request
                // console.log("Sending", request);
                const encoded = msgpack.encode(request);
                new DataView(length_buf).setUint32(0, encoded.byteLength, false);
                await writeAll(this.conn, new Uint8Array(length_buf, 0));
                await writeAll(this.conn, encoded);

                // read response
                const reader = this.conn.readable.getReader({mode: "byob"});
                length_buf = await readFullBuffer(reader, length_buf);
                const response_len = new DataView(length_buf).getUint32(0, false);
                const response_buf = await readFullBuffer(
                    reader,
                    new ArrayBuffer(response_len),
                );
                reader.releaseLock();
                const response = msgpack.decode(
                    new Uint8Array(response_buf)
                ) as MutinyResponse;
                // console.log("Received", response);
                if (response.type === 'Error') {
                    return reject(new Error(response.message))
                };
                resolve(response);
            }
        }
        this.processing = false;
    }

    private request(request: MutinyRequest): Promise<MutinyResponse> {
        const promise = new Promise((resolve, reject) => {
            this.queue.push({request, resolve, reject});
        }) as Promise<MutinyResponse>;
        if (!this.processing) {
            this.processing = true;
            queueMicrotask(() => this.processQueue());
        }
        return promise;
    }

    async localPeerId(): Promise<string> {
        const response = await this.request({type: "LocalPeerId"});
        assert(response.type === 'LocalPeerId');
        return response.peer_id;
    }

    async peers(): Promise<string[]> {
        const response = await this.request({type: "Peers"});
        assert(response.type === 'Peers');
        return response.peers;
    }

    async appInstanceUuid(label: string): Promise<string | null> {
        const response = await this.request({type: "AppInstanceUuid", label});
        assert(response.type === 'AppInstanceUuid');
        return response.uuid;
    }

    async createAppInstance(label: string): Promise<string> {
        const response = await this.request({type: "CreateAppInstance", label});
        assert(response.type === 'CreateAppInstance');
        return response.uuid;
    }

    async messageInvite(peer: string, app_uuid: string): Promise<void> {
        const response = await this.request({type: "MessageInvite", peer, app_uuid});
        assert(response.type === 'Success');
        return;
    }

    async messageInvites(): Promise<MessageInvite[]> {
        const response = await this.request({type: "MessageInvites"});
        assert(response.type === 'MessageInvites');
        return response.invites;
    }

    async messageSend(
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array
    ): Promise<void> {
        const response = await this.request({
            type: "MessageSend", 
            peer, 
            app_uuid,
            from_app_uuid,
            message,
        });
        assert(response.type === 'Success');
        return;
    }

    async messageRead(app_uuid: string): Promise<Message | null> {
        const response = await this.request({type: "MessageRead", app_uuid});
        assert(response.type === 'Message');
        return response.message;
    }

    async messageNext(app_uuid: string): Promise<void> {
        const response = await this.request({type: "MessageNext", app_uuid});
        assert(response.type === 'Success');
        return;
    }

    // async invites(): Promise<{id: string, addr: string}[]> {
    //     const response = await this.request({type: "Invites"});
    //     assert(response.type === 'Invites');
    //     return response.invites;
    // }
}
