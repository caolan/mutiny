import {watch} from "../lib/signaller.js";
import {local_peer_id, selected_invite, messages} from "../state.js";

export default class ChatMessageHistory extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.shadow = this.attachShadow({mode: "open"});
        this.shadow.innerHTML = `
            <link rel="stylesheet" href="style.css">
            <pre id="message-history"></pre>
        `;
        this.history = this.shadow.getElementById('message-history');
        this.stop = watch([local_peer_id, selected_invite, messages], () => {
            this.updateMessages();
        });
        this.updateMessages();
    }

    disconnectedCallback() {
        this.stop();
    }

    updateMessages() {
        let txt = "";
        if (selected_invite.value) {
            for (const msg of messages.value) {
                const match_from = (
                    msg.from.peer === selected_invite.value.peer &&
                    msg.from.app_uuid === selected_invite.value.app_uuid
                );
                const match_to = (
                    msg.to.peer === selected_invite.value.peer &&
                    msg.to.app_uuid === selected_invite.value.app_uuid
                );
                if (match_from || match_to) {
                    const from = msg.from.peer === local_peer_id.value ? 'You' : msg.from.peer;
                    txt += `<${from}>: ${msg.message}\n`;
                }
            }
        }
        this.history.textContent = txt;
    }
}

customElements.define("chat-message-history", ChatMessageHistory);
