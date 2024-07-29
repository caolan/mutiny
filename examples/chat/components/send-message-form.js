import {bind} from "../lib/events.js";
import {watch} from "../lib/signaller.js";
import {appendMessage, local_peer_id, local_app_uuid, selected_announcement} from "../state.js";

export default class ChatSendMessageForm extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.shadow = this.attachShadow({mode: "open"});
        this.shadow.innerHTML = `
            <link rel="stylesheet" href="style.css">
            <form id="send-message-form">
                <input type="text" name="message" placeholder="Type your message&hellip;" />
                <button type="submit">Send</button>
            </form>
        `;
        this.form = this.shadow.getElementById('send-message-form');
        this.input = this.shadow.querySelector('[name=message]');
        this.cleanup = [
            bind(this.form, 'submit', ev => this.submit(ev)),
            watch([selected_announcement], () => this.updateVisibility()),
        ];
    }

    disconnectedCallback() {
        for (const destroy of this.cleanup) destroy();
    }

    updateVisibility() {
        if (selected_announcement.value) {
            this.form.style.display = 'flex';
            this.input.focus();
        } else {
            this.form.style.display = 'none';
        }
    }

    async submit(ev) {
        ev.preventDefault();
        if (selected_announcement.value && local_peer_id.value && local_app_uuid.value) {
            const message = this.input.value;
            await fetch("/_api/v1/messages/outbox", {
                method: "POST",
                body: JSON.stringify({
                    peer: selected_announcement.value.peer,
                    app_uuid: selected_announcement.value.app_uuid,
                    message,
                })
            });
            this.input.value = "";
            const from = {
                peer: local_peer_id.value,
                app_uuid: local_app_uuid.value,
            };
            const to = {
                peer: selected_announcement.value.peer,
                app_uuid: selected_announcement.value.app_uuid,
            };
            appendMessage(from, to, message);
        }
    }
}

customElements.define("chat-send-message-form", ChatSendMessageForm);
