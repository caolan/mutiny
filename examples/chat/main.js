// @ts-check
import * as state from "./state.js";
import {watch} from "./lib/signaller.js";
import {askNick} from "../nick.js";

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

async function updatePeers() {
   const res = await fetch("/_api/v1/peers");
   const current = new Set(await res.json());
   // Announce app to newly discovered peers
   const new_peers = current.difference(state.peers.value);
   announce(new_peers);
   state.peers.value = current;
}

/** @param {Set<string>} peers */
async function announce(peers) {
   const data = {
       id: 'mutiny.example.chat',
       nick: state.nick.value,
   };
   for (const peer of peers) {
       await fetch("/_api/v1/announcements", {
           method: 'POST',
           body: JSON.stringify({peer, data}),
       });
   }
}

async function updateAnnouncements() {
   const res = await fetch("/_api/v1/announcements");
   const data = /** @type {import("../../lib/client.ts").AppAnnouncement[]} */(await res.json());
   // Only list announcements for current app from peers in current discovered list
   const new_announcements = data.filter(x => {
       if (typeof x.data === 'object') {
           return (
               // @ts-ignore typescript doesn't detect that x.data is an object here
               x.data.id === 'mutiny.example.chat' &&
               state.peers.value.has(x.peer)
           );
       }
       return false;
   });
   // Update state only if announcements have changed
   if (JSON.stringify(new_announcements) !== JSON.stringify(state.announcements.value)) {
       state.announcements.value = new_announcements;
   }
}

async function getMessages() {
    while (true) {
        const res = await fetch("/_api/v1/message_read");
        const data = await res.json();
        if (data && state.local_peer_id.value && state.local_app_uuid.value) {
            const from = {
                peer: data.peer,
                app_uuid: data.uuid,
            };
            const to = {
                peer: state.local_peer_id.value,
                app_uuid: state.local_app_uuid.value,
            };
            state.appendMessage(from, to, data.message);
            await fetch("/_api/v1/message_next", {method: "POST"});
        } else {
            // Check again in 1 second
            setTimeout(getMessages, 1000);
            return;
        }
    }
}

// Initialize example app
await updateLocalPeerId();
await updateLocalAppInstance();
await updatePeers();
await updateAnnouncements();

// Update peers when nick changes
watch([state.nick], updatePeers);

// Announce again to all peers when nick changes
watch([state.nick], () => announce(state.peers.value));

// Start polling for messages
getMessages();

// Poll server for new peers and announcements
setInterval(updatePeers, 2000);
setInterval(updateAnnouncements, 2000);

// Ask user for nickname
askNick();
