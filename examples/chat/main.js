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
    const data = await fetch("/_api/v1/application_instance").then(
        res => res.json()
    );
    state.local_app_uuid.value = data.uuid;
}

async function updatePeers() {
   const res = await fetch("/_api/v1/peers");
   const data = new Set(await res.json());
   // Send invites to newly discovered peers
   const new_peers = data.difference(state.peers.value);
   for (const peer of new_peers) {
       await fetch("/_api/v1/message_invite", {
           method: 'POST',
           body: JSON.stringify({peer}),
       });
   }
   state.peers.value = data;
}

async function updateInvites() {
   const res = await fetch("/_api/v1/message_invites");
   const data = /** @type {{peer: string, uuid: string}[]} */(await res.json());
   // Only list invites for peers in current discovered list
   const new_invites = data.filter(x => state.peers.value.has(x.peer));
   // Update state only if invites have changed
   if (JSON.stringify(new_invites) !== JSON.stringify(state.invites.value)) {
       state.invites.value = new_invites;
   }
}

async function getMessages() {
    while (true) {
        const res = await fetch("/_api/v1/message_read");
        const data = await res.json();
        if (data && state.local_peer_id.value && state.local_app_uuid.value) {
            const from = {
                peer: data.peer,
                app_instance_uuid: data.uuid,
            };
            const to = {
                peer: state.local_peer_id.value,
                app_instance_uuid: state.local_app_uuid.value,
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
// renderInvites();
await updateLocalPeerId();
await updateLocalAppInstance();
await updatePeers();
await updateInvites();

// Start polling for messages
getMessages();

// Poll server for new peers and invites
setInterval(updatePeers, 2000);
setInterval(updateInvites, 2000);
