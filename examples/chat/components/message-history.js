import {watch} from "../lib/signaller.js";
import {local_peer_id, nick, announcements, selected_announcement, messages} from "../state.js";

export default class ChatMessageHistory extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.stop = watch([
            local_peer_id, 
            selected_announcement, 
            announcements,
            messages,
        ], () => this.updateMessages());
        this.updateMessages();
    }

    disconnectedCallback() {
        this.stop();
    }

    getNick(peer, app_uuid) {
        if (peer === local_peer_id.value) {
            return nick.value || 'You';
        }
        for (const announcement of announcements.value) {
            if (announcement.peer === peer && announcement.app_uuid === app_uuid) {
                return announcement.data.nick || peer;
            }
        }
        return peer;
    }

    updateMessages() {
        let txt = "";
        if (selected_announcement.value) {
            for (const msg of messages.value) {
                const match_from = (
                    msg.from.peer === selected_announcement.value.peer &&
                    msg.from.app_uuid === selected_announcement.value.app_uuid
                );
                const match_to = (
                    msg.to.peer === selected_announcement.value.peer &&
                    msg.to.app_uuid === selected_announcement.value.app_uuid
                );
                if (match_from || match_to) {
                    txt += `<${this.getNick(msg.from.peer, msg.from.app_uuid)}>: ${msg.message}\n`;
                }
            }
        }
        this.textContent = txt;
    }
}

customElements.define("chat-message-history", ChatMessageHistory);
