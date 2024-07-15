import {bind} from "../lib/events.js";
import {watch} from "../lib/signaller.js";
import {local_peer_id, nick} from "../state.js";
import {askNick} from "../nick.js";

export default class ChatHeader extends HTMLElement {
    connectedCallback() {
        this.innerHTML = `
            <header>
                <h1>Chat Example</h1>
                <a href="#" id="nick"></a>
            </header>
        `;
        this.a = this.querySelector('#nick');
        this.cleanup = [
            watch([local_peer_id, nick], () => this.updateText()),
            bind(this.a, 'click', ev => {
                ev.preventDefault();
                askNick();
            }),
        ];
        this.updateText();
    }

    updateText() {
        const text = nick.value || local_peer_id.value;
        if (text) {
            this.a.textContent = `You: ${text}`;
            this.a.style.visibility = 'visible';
        } else {
            this.a.style.visibility = 'hidden';
        }
    }

    disconnectedCallback() {
        for (const destroy of this.cleanup) destroy();
    }
}

customElements.define("chat-header", ChatHeader);
