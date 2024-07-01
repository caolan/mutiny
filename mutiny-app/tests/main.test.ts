import { join } from "@std/path/join";
import { assertEquals } from "@std/assert";
import { MutinyClient } from "../../lib/client.ts";
import { Server }from "../src/main.ts";

const BASE_URL = "http://localhost:8000";
const ROOT = join(import.meta.dirname as string, "www");

const INSTANCE = {
    uuid: "1234567890",
    name: "Example app",
};

function makeServer(client: unknown) {
    return new Server(client as MutinyClient, INSTANCE, ROOT);
}

Deno.test("Get app instance information", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/_api/v1/application_instance`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, INSTANCE);
});

Deno.test("Get local peer id", async () => {
    const server = makeServer({
        localPeerId() {
            return Promise.resolve("123abc");
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/local_peer_id`);
    const response = await server.handleRequest(request);
    const data = await response.text();
    assertEquals(data, "123abc");
});

Deno.test("Get peers list", async () => {
    const peers = ["peer1", "peer2", "peer3"];
    const server = makeServer({
        peers() {
            return Promise.resolve(peers);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/peers`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, peers);
});

Deno.test("Send message invite", async () => {
    const calls: [string, string][] = [];
    const server = makeServer({
        messageInvite(peer: string, uuid: string) {
            calls.push([peer, uuid]);
            return Promise.resolve(undefined);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/message_invite`, {
        method: "POST",
        body: JSON.stringify({
            peer: "peer2", 
            app_instance_uuid: "app2",
        }),
    });
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, {success: true});
    assertEquals(calls, [["peer2", INSTANCE.uuid]]);
});

Deno.test("List message invites", async () => {
    const invites = [
        {
            peer: "peer1",
            app_instance_uuid: "app1",
            manifest_id: "example.app.one",
            manifest_version: "1.1.1",
        },
        {
            peer: "peer2",
            app_instance_uuid: "app2",
            manifest_id: "example.app.two",
            manifest_version: "2.2.2",
        },
    ];
    const server = makeServer({
        messageInvites() {
            return Promise.resolve(invites);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/message_invites`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, invites);
});

Deno.test("Read message (with message)", async () => {
    const message = {
        peer: "peer2",
        uuid: "app2",
        message: new TextEncoder().encode("hello"),
    };
    const server = makeServer({
        messageRead(uuid: string) {
            assertEquals(uuid, INSTANCE.uuid);
            return Promise.resolve(message);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/message_read`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, {
        peer: "peer2",
        uuid: "app2",
        message: "hello",
    });
});

Deno.test("Read message (with no message)", async () => {
    const server = makeServer({
        messageRead(uuid: string) {
            assertEquals(uuid, INSTANCE.uuid);
            return Promise.resolve(null);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/message_read`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, null);
});

Deno.test("Next message", async () => {
    const calls: string[] = [];
    const server = makeServer({
        messageNext(uuid: string) {
            calls.push(uuid);
            return Promise.resolve(null);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/message_next`, {
        method: "POST",
    });
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, {success: true});
    assertEquals(calls, [INSTANCE.uuid]);
});

Deno.test("Unknown API path", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/_api/v1/not_found`);
    const response = await server.handleRequest(request);
    assertEquals(response.status, 404);
});

Deno.test("Get static file from app root directory", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/example.txt`);
    const response = await server.handleRequest(request);
    assertEquals(response.status, 200);
    const data = await response.text();
    assertEquals(data, "Hello, world!\n");
});
