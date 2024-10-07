// @ts-check
import * as state from "./state.js";
import {watch} from "./lib/signaller.js";
import {askNick} from "./nick.js";

// Custom elements
import "./components/header.js";
import "./components/peers.js";
import "./components/message-history.js";
import "./components/send-message-form.js";

async function updateLocalPeerId() {
    state.local_peer_id.value = await fetch("/_api/v1/local_peer_id").then(
        res => res.text()
    );
}

async function updateLocalAppInstance() {
    const data = await fetch("/_api/v1/application").then(
        res => res.json()
    );
    state.local_app_uuid.value = data.uuid;
}

async function fetchPeers() {
   const res = await fetch("/_api/v1/peers");
   return new Set(await res.json());
}

/** @param {Set<string>} peers */
async function announce(peers) {
   const data = {
       id: 'mutiny.example.chat',
       nick: state.nick.value,
   };
   for (const peer of peers) {
       await fetch("/_api/v1/announcements/outbox", {
           method: 'POST',
           body: JSON.stringify({peer, data}),
       });
   }
}

async function fetchAnnouncements() {
   const res = await fetch("/_api/v1/announcements/inbox");
   const data = /** @type {import("../../lib/client.ts").AppAnnouncement[]} */(await res.json());
   // Only list announcements for current app from peers in current discovered list
   return data.filter(x => {
       if (typeof x.data === 'object') {
           // @ts-ignore typescript doesn't detect that x.data is an object here
           return x.data.id === 'mutiny.example.chat';
       }
       return false;
   });
}

/** @param {import("../../lib/client.ts").MessageJson} message */
async function receiveMessage(message) {
    if (state.local_peer_id.value && state.local_app_uuid.value) {
        const from = {
            peer: message.peer,
            app_uuid: message.uuid,
        };
        const to = {
            peer: state.local_peer_id.value,
            app_uuid: state.local_app_uuid.value,
        };
        state.appendMessage(from, to, message.message);
        // Delete seen messages
        await fetch("/_api/v1/messages/inbox", {
            method: "DELETE",
            body: JSON.stringify({
                message_id: message.id
            }),
        });
    }
}

async function fetchMessages() {
    const res = await fetch("/_api/v1/messages/inbox");
    const data = await res.json();
    for (const message of data) {
        await receiveMessage(message);
    }
}

// Initialize example app
await updateLocalPeerId();
await updateLocalAppInstance();
state.announcements.value = await fetchAnnouncements();
state.peers.value = await fetchPeers();
await fetchMessages();

// Announce app to newly discovered peers
announce(state.peers.value);

// Announce to all peers when nick changes
watch([state.nick], () => announce(state.peers.value));

// Ask user for nickname
askNick();

// Subscribe to server-sent events
const peer_events = new EventSource("/_api/v1/peers/events");
peer_events.addEventListener("PeerDiscovered", peer_id => {
    console.log('Peer discovered', peer_id);
    state.peers.value.add(peer_id);
    state.peers.signal();
});
peer_events.addEventListener("PeerExpired", peer_id => {
    console.log('Peer expired', peer_id);
    state.peers.value.delete(peer_id);
    state.peers.signal();
});

const announcement_events = new EventSource("/_api/v1/announcements/inbox/events");
announcement_events.addEventListener("AppAnnouncement", event => {
    const announcement = JSON.parse(event.data);
    console.log('App announced', announcement);
    state.announcements.value = state.announcements.value.map(x => {
        if (x.peer === announcement.peer || x.app_uuid === announcement.app_uuid) {
            return announcement;
        }
        return x;
    });
});

const inbox_events = new EventSource("/_api/v1/messages/inbox/events");
inbox_events.addEventListener("Message", event => {
    const message = JSON.parse(event.data);
    console.log('Message received', message);
    receiveMessage(/** @type {import("../../lib/client.ts").MessageJson} */(message));
});
