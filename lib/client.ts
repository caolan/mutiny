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

interface JsonObject { [name: string]: JsonValue }
interface JsonArray extends Array<JsonValue> { }
type JsonValue = (null | boolean | number | string | JsonObject | JsonArray);

export type Message = {
    type: "Message",
    id: number,
    peer: string,
    uuid: string,
    message: Uint8Array,
};

export type MessageJson = {
    type: "Message",
    id: number,
    peer: string,
    uuid: string,
    message: string,
};

export type AppAnnouncement = {
    type: "AppAnnouncement",
    peer: string, 
    app_uuid: string,
    data: JsonValue,
};

export type MutinyRequest = {
    id: number,
    body: MutinyRequestBody,
};
export type MutinyRequestBody = {type: "LocalPeerId"}
    | {type: "Peers"}
    | {type: "AppAnnouncements"}
    | {type: "GetLastPort", app_uuid: string}
    | {type: "SetLastPort", app_uuid: string, port: number}
    | {type: "AppInstanceUuid", label: string}
    | {type: "CreateAppInstance", label: string}
    | {type: "Announce", peer: string, app_uuid: string, data: JsonValue}
    | {type: "AppAnnouncements"}
    | {
        type: "SendMessage", 
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array,
    }
    | {type: "InboxMessages", app_uuid: string}
    | {type: "DeleteInboxMessage", app_uuid: string, message_id: number}
    | {type: "SubscribePeerEvents"}
    | {type: "SubscribeAnnounceEvents"}
    | {type: "SubscribeInboxEvents", app_uuid: string}
    ;

export type MutinyResponse = {
    request_id: number,
    body: MutinyResponseBody,
};

export type PeerEvent = {type: "PeerDiscovered", peer_id: string}
    | {type: "PeerExpired", peer_id: string};

export type MutinyResponseBody = {type: "Success"} 
    | {type: "Error", message: string}
    | {type: "LocalPeerId", peer_id: string}
    | {type: "Peers", peers: string[]}
    | {type: "AppInstanceUuid", uuid: string | null}
    | {type: "GetLastPort", port: number | null}
    | {type: "CreateAppInstance", uuid: string}
    | {type: "Message", message: Message}
    | {type: "InboxMessages", messages: Message[]}
    | {type: "AppAnnouncements",  announcements: AppAnnouncement[]}
    | PeerEvent
    ;

export class MutinyClient {
    private next_request_id = 1;
    private sending_requests = false;
    private dispatching_responses = false;
    private queued: MutinyRequest[] = [];
    private waiting: Map<number, {
        resolve: (response: MutinyResponseBody) => void,
        reject: (err: Error) => void,
    }> = new Map();

    constructor(
        private conn: Deno.UnixConn,
    ) {}

    private async sendRequests() {
        while (this.queued.length > 0) {
            const requests = this.queued;
            this.queued = [];
            for (const request of requests) {
                // send request
                console.log("Sending", request);
                const length_buf = new ArrayBuffer(4);
                const encoded = msgpack.encode(request);
                new DataView(length_buf).setUint32(0, encoded.byteLength, false);
                await writeAll(this.conn, new Uint8Array(length_buf, 0));
                await writeAll(this.conn, encoded);
            }
        }
        this.sending_requests = false;
    }

    private async dispatchResponses() {
        while (this.waiting.size > 0) {
            // read response
            let length_buf = new ArrayBuffer(4);
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
            console.log("Received", response);
            const req = this.waiting.get(response.request_id);
            if (req) {
                // Synchronous delete allows while loop to exit
                // when all requests have been responded to - in
                // the case of requests with multiple responses,
                // it is necessary to register the next promise
                // and call dispatchResponses again.
                this.waiting.delete(response.request_id);
                if (response.body.type === 'Error') {
                    return req.reject(new Error(response.body.message))
                };
                req.resolve(response.body);
            }
        }
        this.dispatching_responses = false;
    }

    private queueRequest(request: MutinyRequest): Promise<MutinyResponseBody> {
        // Create promise and register with request id
        const promise = new Promise((resolve, reject) => {
            this.waiting.set(request.id, {resolve, reject});
        }) as Promise<MutinyResponseBody>;

        // Queue request
        this.queued.push(request);

        // Start sending queued requests (if not already)
        this.queueSendingRequests();
        // Start dispatching responses (if not already)
        this.queueDispatchResponses();
        return promise;
    }

    queueDispatchResponses() {
        if (!this.dispatching_responses) {
            this.dispatching_responses = true;
            queueMicrotask(() => this.dispatchResponses());
        }
    }

    queueSendingRequests() {
        if (!this.sending_requests) {
            this.sending_requests = true;
            queueMicrotask(() => this.sendRequests());
        }
    }

    private async requestOne(body: MutinyRequestBody): Promise<MutinyResponseBody> {
        const request = {id: this.next_request_id++, body};
        const response = await this.queueRequest(request);
        // No more responses expected so no need to register new promise
        return response;
    }

    async localPeerId(): Promise<string> {
        const response = await this.requestOne({type: "LocalPeerId"});
        assert(response.type === 'LocalPeerId');
        return response.peer_id;
    }

    async peers(): Promise<string[]> {
        const response = await this.requestOne({type: "Peers"});
        assert(response.type === 'Peers');
        return response.peers;
    }

    private _subscribe<R>(request: MutinyRequest): AsyncIterableIterator<R> {
        const waiting = this.waiting;
        let promise: Promise<MutinyResponseBody> = this.queueRequest(request);
        return {
            [Symbol.asyncIterator]() {
                return this;
            },
            return(value?: PeerEvent) {
                // Remove waiting promise
                waiting.delete(request.id);
                return Promise.resolve({value, done: true});
            },
            next: async () => {
                const value = await promise;
                // Register next promise
                promise = new Promise((resolve, reject) => {
                    waiting.set(request.id, {resolve, reject});
                }) as Promise<MutinyResponseBody>;
                // Start dispatching responses (if not already)
                this.queueDispatchResponses();
                return {value: value as R};
            }
        };
    }

    peerEvents(): AsyncIterableIterator<PeerEvent> {
        const body: MutinyRequestBody = {type: "SubscribePeerEvents"};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    announceEvents(): AsyncIterableIterator<AppAnnouncement> {
        const body: MutinyRequestBody = {type: "SubscribeAnnounceEvents"};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    inboxEvents(app_uuid: string): AsyncIterableIterator<Message> {
        const body: MutinyRequestBody = {type: "SubscribeInboxEvents", app_uuid};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    async appInstanceUuid(label: string): Promise<string | null> {
        const response = await this.requestOne({type: "AppInstanceUuid", label});
        assert(response.type === 'AppInstanceUuid');
        return response.uuid;
    }

    async getLastPort(app_uuid: string): Promise<number | null> {
        const response = await this.requestOne({type: "GetLastPort", app_uuid});
        assert(response.type === 'GetLastPort');
        return response.port;
    }

    async setLastPort(app_uuid: string, port: number): Promise<void> {
        const response = await this.requestOne({type: "SetLastPort", app_uuid, port});
        assert(response.type === 'Success');
    }

    async createAppInstance(label: string): Promise<string> {
        const response = await this.requestOne({type: "CreateAppInstance", label});
        assert(response.type === 'CreateAppInstance');
        return response.uuid;
    }

    async announce(peer: string, app_uuid: string, data: JsonValue): Promise<void> {
        const response = await this.requestOne({type: "Announce", peer, app_uuid, data});
        assert(response.type === 'Success');
    }

    async announcements(): Promise<AppAnnouncement[]> {
        const response = await this.requestOne({type: "AppAnnouncements"});
        assert(response.type === 'AppAnnouncements');
        return response.announcements;
    }

    async sendMessage(
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array
    ): Promise<void> {
        const response = await this.requestOne({
            type: "SendMessage", 
            peer, 
            app_uuid,
            from_app_uuid,
            message,
        });
        assert(response.type === 'Success');
        return;
    }

    async inboxMessages(app_uuid: string): Promise<Message[]> {
        const response = await this.requestOne({type: "InboxMessages", app_uuid});
        assert(response.type === 'InboxMessages');
        return response.messages;
    }

    async deleteInboxMessage(app_uuid: string, message_id: number): Promise<void> {
        const response = await this.requestOne({type: "DeleteInboxMessage", app_uuid, message_id});
        assert(response.type === 'Success');
        return;
    }
}
