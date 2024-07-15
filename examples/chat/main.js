// @ts-check
import * as state from "./state.js";

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
   const data = {};
   for (const peer of new_peers) {
       await fetch("/_api/v1/announcements", {
           method: 'POST',
           body: JSON.stringify({peer, data}),
       });
   }
   state.peers.value = current;
}

async function updateAnnouncements() {
   const res = await fetch("/_api/v1/announcements");
   const data = /** @type {{peer: string, uuid: string}[]} */(await res.json());
   // Only list announcements for peers in current discovered list
   const new_announcements = data.filter(x => state.peers.value.has(x.peer));
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

// Start polling for messages
getMessages();

// Poll server for new peers and announcements
setInterval(updatePeers, 2000);
setInterval(updateAnnouncements, 2000);
